// ScreenCaptureKit capture has no selection UI or desktop dimming overlay.
// Runs inside the signed accessibility helper until SIGINT or duration expiry.
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <AVFoundation/AVFoundation.h>
#include <signal.h>
#include <poll.h>
#include <unistd.h>

static volatile sig_atomic_t cuStopRecording = 0;
static BOOL cuRecordingOwnerPipe = NO;
static void cuRecordingSignal(int sig) { cuStopRecording = 1; }
static BOOL cuRecordingOwnerClosed(void) {
  if(!cuRecordingOwnerPipe) return NO;
  struct pollfd fd={STDIN_FILENO,POLLHUP,0};
  return poll(&fd,1,0)>0 && (fd.revents&POLLHUP);
}

@interface CURecorder : NSObject <SCStreamOutput, SCStreamDelegate>
@property AVAssetWriter *writer;
@property AVAssetWriterInput *input;
@property NSError *failure;
@property BOOL started;
@end
@implementation CURecorder
- (void)stream:(SCStream *)stream didStopWithError:(NSError *)error {
  self.failure=error; cuStopRecording=1;
}
- (void)stream:(SCStream *)stream didOutputSampleBuffer:(CMSampleBufferRef)sample ofType:(SCStreamOutputType)type {
  if(type!=SCStreamOutputTypeScreen || !CMSampleBufferIsValid(sample)) return;
  NSArray *attachments=(__bridge NSArray *)CMSampleBufferGetSampleAttachmentsArray(sample,NO);
  if(!attachments.count || [attachments[0][SCStreamFrameInfoStatus] integerValue]!=SCFrameStatusComplete) return;
  if(!self.started) {
    if(![self.writer startWriting]) { self.failure=self.writer.error; cuStopRecording=1; return; }
    [self.writer startSessionAtSourceTime:CMSampleBufferGetPresentationTimeStamp(sample)];
    self.started=YES;
  }
  if(self.input.readyForMoreMediaData && ![self.input appendSampleBuffer:sample]) {
    self.failure=self.writer.error; cuStopRecording=1;
  }
}
@end

static void cuWait(dispatch_semaphore_t sem, NSTimeInterval seconds, BOOL starting) {
  NSDate *deadline=[NSDate dateWithTimeIntervalSinceNow:seconds];
  while(dispatch_semaphore_wait(sem,DISPATCH_TIME_NOW)!=0) {
    if(starting && (cuStopRecording || cuRecordingOwnerClosed()))
      @throw [NSException exceptionWithName:@"cancelled" reason:@"recording owner closed during startup; partial file retained" userInfo:nil];
    if(deadline.timeIntervalSinceNow<=0) @throw [NSException exceptionWithName:@"recording" reason:@"screen recorder timed out" userInfo:nil];
    [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
  }
}
static id cuRecord(NSDictionary *args) {
  cuRecordingOwnerPipe=[args[@"owner_pipe"] boolValue];
  signal(SIGINT,cuRecordingSignal);signal(SIGTERM,cuRecordingSignal);
  if(!CGPreflightScreenCaptureAccess()) @throw [NSException exceptionWithName:@"permission" reason:@"Screen Recording permission is missing" userInfo:nil];
  NSString *file=args[@"file"];
  if(![file isKindOfClass:NSString.class] || ![file isAbsolutePath] || [[NSFileManager defaultManager] fileExistsAtPath:file])
    @throw [NSException exceptionWithName:@"recording" reason:@"recording needs a new absolute output path" userInfo:nil];
  __block SCShareableContent *content; __block NSError *error;
  dispatch_semaphore_t sem=dispatch_semaphore_create(0);
  [SCShareableContent getShareableContentExcludingDesktopWindows:NO onScreenWindowsOnly:YES completionHandler:^(SCShareableContent *c,NSError *e){ content=c;error=e;dispatch_semaphore_signal(sem); }];
  cuWait(sem,15,YES);
  if(error) @throw [NSException exceptionWithName:@"recording" reason:error.localizedDescription userInfo:nil];
  SCDisplay *display=nil;
  for(SCDisplay *d in content.displays) if(d.displayID==[args[@"displayID"] unsignedIntValue]) display=d;
  if(!display) @throw [NSException exceptionWithName:@"recording" reason:@"recording display is unavailable" userInfo:nil];
  CGRect bounds=CGDisplayBounds(display.displayID), crop=CGRectMake(0,0,bounds.size.width,bounds.size.height);
  NSArray *region=args[@"region"];
  if(region) {
    if(region.count!=4) @throw [NSException exceptionWithName:@"recording" reason:@"region must be [x,y,w,h]" userInfo:nil];
    crop=CGRectMake([region[0] doubleValue]-bounds.origin.x,[region[1] doubleValue]-bounds.origin.y,[region[2] doubleValue],[region[3] doubleValue]);
    if(!isfinite(crop.origin.x)||!isfinite(crop.origin.y)||!isfinite(crop.size.width)||!isfinite(crop.size.height)||crop.size.width<=0||crop.size.height<=0||!CGRectContainsRect(CGRectMake(0,0,bounds.size.width,bounds.size.height),crop))
      @throw [NSException exceptionWithName:@"recording" reason:@"recording region must fit the selected display" userInfo:nil];
  }
  SCStreamConfiguration *config=[SCStreamConfiguration new];
  config.sourceRect=crop;
  // Even dimensions for H.264. Keep point-sized output to bound encoder cost.
  config.width=MAX(2,((size_t)crop.size.width/2)*2);config.height=MAX(2,((size_t)crop.size.height/2)*2);
  config.minimumFrameInterval=CMTimeMake(1,30);config.showsCursor=NO;config.capturesAudio=NO;
  CURecorder *rec=[CURecorder new];
  rec.writer=[[AVAssetWriter alloc] initWithURL:[NSURL fileURLWithPath:file] fileType:AVFileTypeQuickTimeMovie error:&error];
  if(!rec.writer) @throw [NSException exceptionWithName:@"recording" reason:error.localizedDescription userInfo:nil];
  rec.input=[AVAssetWriterInput assetWriterInputWithMediaType:AVMediaTypeVideo outputSettings:@{AVVideoCodecKey:AVVideoCodecTypeH264,AVVideoWidthKey:@(config.width),AVVideoHeightKey:@(config.height)}];
  rec.input.expectsMediaDataInRealTime=YES;
  if(![rec.writer canAddInput:rec.input]) @throw [NSException exceptionWithName:@"recording" reason:@"video encoder unavailable" userInfo:nil];
  [rec.writer addInput:rec.input];
  dispatch_queue_t queue=dispatch_queue_create("net.codewhale.recording",DISPATCH_QUEUE_SERIAL);
  SCContentFilter *filter=[[SCContentFilter alloc] initWithDisplay:display excludingWindows:@[]];
  SCStream *stream=[[SCStream alloc] initWithFilter:filter configuration:config delegate:rec];
  if(![stream addStreamOutput:rec type:SCStreamOutputTypeScreen sampleHandlerQueue:queue error:&error])
    @throw [NSException exceptionWithName:@"recording" reason:error.localizedDescription userInfo:nil];
  if(cuStopRecording || cuRecordingOwnerClosed())
    @throw [NSException exceptionWithName:@"cancelled" reason:@"recording owner closed before capture started" userInfo:nil];
  [stream startCaptureWithCompletionHandler:^(NSError *e){error=e;dispatch_semaphore_signal(sem);}];cuWait(sem,15,YES);
  if(error) @throw [NSException exceptionWithName:@"recording" reason:error.localizedDescription userInfo:nil];
  puts("{\"ready\":true}");fflush(stdout);
  double duration=[args[@"durationSec"] doubleValue];
  NSDate *deadline=duration>0?[NSDate dateWithTimeIntervalSinceNow:duration]:nil;
  while(!cuStopRecording && !cuRecordingOwnerClosed() && (!deadline || deadline.timeIntervalSinceNow>0))
    [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
  [stream stopCaptureWithCompletionHandler:^(NSError *e){error=e;dispatch_semaphore_signal(sem);}];cuWait(sem,15,NO);
  dispatch_sync(queue,^{}); // Drain frames before finalizing the file.
  if(error || rec.failure || !rec.started) {
    [rec.writer cancelWriting];
    @throw [NSException exceptionWithName:@"recording" reason:(error?:rec.failure).localizedDescription?:@"no complete video frames received" userInfo:nil];
  }
  [rec.input markAsFinished];
  [rec.writer finishWritingWithCompletionHandler:^{dispatch_semaphore_signal(sem);}];cuWait(sem,15,NO);
  if(rec.writer.status!=AVAssetWriterStatusCompleted) @throw [NSException exceptionWithName:@"recording" reason:rec.writer.error.localizedDescription?:@"video finalization failed" userInfo:nil];
  return @{@"finished":@YES,@"width":@(config.width),@"height":@(config.height)};
}
