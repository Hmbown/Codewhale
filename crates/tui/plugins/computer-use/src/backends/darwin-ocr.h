#import <Cocoa/Cocoa.h>
#import <Vision/Vision.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

// Text recognition enriches an existing window observation. It neither reads
// the desktop nor creates accessibility roles or element identities.
static NSDictionary *cuOCRUnavailable(NSString *reason) {
  return @{ @"status":@"unavailable", @"engine":@"apple_vision", @"reason":reason, @"blocks":@[] };
}

static NSDictionary *cuOCRPixelBounds(CGRect box, size_t width, size_t height) {
  // Vision uses a normalized lower-left origin; raster targets use upper-left.
  double x=MAX(0,MIN(width,floor(box.origin.x*width)));
  double y=MAX(0,MIN(height,floor((1-CGRectGetMaxY(box))*height)));
  double right=MAX(x,MIN(width,ceil(CGRectGetMaxX(box)*width)));
  double bottom=MAX(y,MIN(height,ceil((1-box.origin.y)*height)));
  return @{ @"x":@(x), @"y":@(y), @"w":@(right-x), @"h":@(bottom-y) };
}

static NSDictionary *cuRecognizeText(NSString *file) {
  if(![file isKindOfClass:NSString.class] || !file.isAbsolutePath)
    return cuOCRUnavailable(@"OCR needs the captured window image");
  int fd=open(file.fileSystemRepresentation,O_RDONLY|O_NOFOLLOW|O_CLOEXEC);
  struct stat st;
  if(fd<0) return cuOCRUnavailable(@"The captured window image could not be opened");
  if(fstat(fd,&st)!=0 || !S_ISREG(st.st_mode) || st.st_size<=0 || st.st_size>64*1024*1024) {
    close(fd); return cuOCRUnavailable(@"The captured window image is empty or exceeds the 64 MiB OCR limit");
  }
  // Read the same descriptor we checked; do not reopen a replaceable path.
  NSMutableData *bytes=[NSMutableData dataWithLength:(NSUInteger)st.st_size];
  size_t offset=0;
  while(offset<bytes.length) {
    ssize_t count=read(fd,(char *)bytes.mutableBytes+offset,bytes.length-offset);
    if(count<0 && errno==EINTR) continue;
    if(count<=0) { close(fd); return cuOCRUnavailable(@"The captured window image could not be read"); }
    offset+=(size_t)count;
  }
  close(fd);
  NSBitmapImageRep *bitmap=[[NSBitmapImageRep alloc] initWithData:bytes];
  CGImageRef image=bitmap.CGImage;
  if(!image) return cuOCRUnavailable(@"The captured window image could not be decoded");
  size_t width=CGImageGetWidth(image),height=CGImageGetHeight(image);
  if(!width || !height || width>50000000/height)
    return cuOCRUnavailable(@"The captured window image exceeds the 50 megapixel OCR limit");
  @try {
    VNRecognizeTextRequest *request=[VNRecognizeTextRequest new];
    request.recognitionLevel=VNRequestTextRecognitionLevelAccurate;
    // Preserve literal UI strings, including code, instead of correcting words.
    request.usesLanguageCorrection=NO;
    if(@available(macOS 13.0,*)) request.automaticallyDetectsLanguage=YES;
    VNImageRequestHandler *handler=[[VNImageRequestHandler alloc] initWithCGImage:image options:@{}];
    NSError *error=nil;
    if(![handler performRequests:@[request] error:&error])
      return cuOCRUnavailable([NSString stringWithFormat:@"Apple Vision could not recognize this image: %@",error.localizedDescription?:@"unknown error"]);
    NSMutableArray *blocks=[NSMutableArray array];
    NSUInteger characters=0;
    BOOL truncated=NO;
    for(VNRecognizedTextObservation *observation in request.results) {
      VNRecognizedText *candidate=[observation topCandidates:1].firstObject;
      if(!candidate.string.length) continue;
      if(blocks.count>=256 || characters+candidate.string.length>16000) { truncated=YES; break; }
      CGRect box=observation.boundingBox;
      if(!isfinite(box.origin.x) || !isfinite(box.origin.y) || !isfinite(box.size.width) || !isfinite(box.size.height)) continue;
      NSDictionary *bounds=cuOCRPixelBounds(box,width,height);
      if([bounds[@"w"] doubleValue]<=0 || [bounds[@"h"] doubleValue]<=0) continue;
      [blocks addObject:@{ @"text":candidate.string, @"confidence":@(candidate.confidence), @"bounds":bounds }];
      characters+=candidate.string.length;
    }
    return @{ @"status":@"ok", @"engine":@"apple_vision", @"coordinate_space":@"raster_pixels",
              @"pixels":@{ @"w":@(width), @"h":@(height) }, @"blocks":blocks,
              @"truncated":@(truncated), @"character_count":@(characters) };
  } @catch(NSException *error) {
    return cuOCRUnavailable([NSString stringWithFormat:@"Apple Vision is unavailable: %@",error.reason?:error.name]);
  }
}
