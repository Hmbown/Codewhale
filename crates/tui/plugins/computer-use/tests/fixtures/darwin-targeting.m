// Use the production resolver with a synthetic workspace. The fixture entry
// point never calls execute(), AX observation, activation, capture, or input.
#import <Cocoa/Cocoa.h>
@interface CUFixtureApplication : NSObject
@property pid_t processIdentifier;
@property NSString *localizedName;
@property NSString *bundleIdentifier;
@property(getter=isTerminated) BOOL terminated;
@end
@implementation CUFixtureApplication
@end
@interface CUFixtureWorkspace : NSObject
+ (instancetype)sharedWorkspace;
- (NSArray *)runningApplications;
- (NSRunningApplication *)frontmostApplication;
@end
@implementation CUFixtureWorkspace
+ (instancetype)sharedWorkspace { static id workspace; if(!workspace) workspace=[self new]; return workspace; }
- (NSArray *)runningApplications {
  CUFixtureApplication *target=[CUFixtureApplication new], *front=[CUFixtureApplication new];
  target.processIdentifier=123; target.localizedName=@"Fixture"; target.bundleIdentifier=@"test.fixture";
  front.processIdentifier=999; front.localizedName=@"Other"; front.bundleIdentifier=@"test.other";
  return @[front,target];
}
- (NSRunningApplication *)frontmostApplication { return (NSRunningApplication *)self.runningApplications[0]; }
@end
#define NSWorkspace CUFixtureWorkspace
#import <ApplicationServices/ApplicationServices.h>
static int fixturePosts=0, fixtureActivations=0;
static void fixturePost(CGEventTapLocation location, CGEventRef event) { fixturePosts++; }
static AXError fixtureSet(AXUIElementRef element, CFStringRef name, CFTypeRef value) { fixtureActivations++; return kAXErrorSuccess; }
#define CGEventPost fixturePost
#define AXUIElementSetAttributeValue fixtureSet
#define AXIsProcessTrusted() true
#define CU_TEST 1
#define main unusedNativeMain
#include "../../src/backends/darwin-accessibility.m"
#undef main
#undef NSWorkspace

int main(int argc, const char *argv[]) { @autoreleasepool {
  if(argc!=2) return 2;
  NSDictionary *request=[NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:argv[1]] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
  if([request[@"tool"] isEqual:@"inspect_pointer_guard"]) {
    cuTestLockDir=request[@"args"][@"lock_dir"];
    NSString *refusal=nil;
    @try { execute(@{@"tool":@"pointer_sequence",@"args":@{@"foreground_input":@YES,@"input_app_ref":@{@"pid":@123},@"steps":@[@{@"type":@1,@"x":@10,@"y":@10,@"button":@0}]}}); }
    @catch(NSException *error) { refusal=error.reason; }
    cuPrint(@{@"refusal":refusal?:NSNull.null,@"posts":@(fixturePosts),@"activations":@(fixtureActivations)});
    return 0;
  }
  if(![@[@"list_windows",@"get_app_state",@"resolve_element",@"window_info"] containsObject:request[@"tool"]]) return 2;
  NSRunningApplication *app=resolve(request[@"args"][@"app_ref"]);
  if(!app) { fputs("application not found\n",stderr); return 1; }
  NSDictionary *receipt=@{@"found":@YES,@"pid":@(app.processIdentifier),@"name":app.localizedName,@"bundle_id":app.bundleIdentifier,@"windows":@[],@"elements":@[]};
  NSData *json=[NSJSONSerialization dataWithJSONObject:receipt options:0 error:nil];
  fwrite(json.bytes,1,json.length,stdout); fputc('\n',stdout);
  return 0;
} }
