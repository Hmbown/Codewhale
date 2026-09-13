#import <Cocoa/Cocoa.h>
#import <ApplicationServices/ApplicationServices.h>
#include <unistd.h>
#include <signal.h>
#include <poll.h>
#include <fcntl.h>
#include <sys/file.h>
#include <sys/stat.h>
#import "darwin-recording.h"
#import "darwin-ocr.h"

static volatile sig_atomic_t cuCancelled = 0;
static BOOL cuOwnerPipe = NO;
static NSDictionary *cuLeaseKey = nil;
static pid_t cuLeasePid = 0;
static NSRunningApplication *cuLeaseApp = nil;
static BOOL cuLeaseButtons[3] = {NO,NO,NO};
static CGPoint cuLeasePoint;
#ifdef CU_TEST
static NSString *cuTestLockDir = nil;
static NSString *cuTestReleaseFile = nil;
static CGEventFlags cuTestInheritedTextFlags = 0;
#endif
static void cuCancel(int signum) { cuCancelled = 1; }
static void cuCheckCancelled(void) {
  if(cuOwnerPipe) { struct pollfd fd={STDIN_FILENO,POLLHUP,0}; if(poll(&fd,1,0)>0 && (fd.revents&POLLHUP)) cuCancelled=1; }
  if(cuCancelled) @throw [NSException exceptionWithName:@"cancelled" reason:@"computer request cancelled" userInfo:nil];
}
static void cuLockInput(void) {
  // One physical desktop, including separately launched direct MCP hosts.
  // The kernel releases this lock if the native owner itself crashes.
  NSString *dir=[NSHomeDirectory() stringByAppendingPathComponent:@".codewhale-cu"];
#ifdef CU_TEST
  if(cuTestLockDir) dir=cuTestLockDir;
#endif
  [NSFileManager.defaultManager createDirectoryAtPath:dir withIntermediateDirectories:YES attributes:@{NSFilePosixPermissions:@0700} error:nil];
  int fd=open([[dir stringByAppendingPathComponent:@"input.lock"] fileSystemRepresentation],O_CREAT|O_RDWR|O_NOFOLLOW|O_CLOEXEC,0600);
  if(fd<0) @throw [NSException exceptionWithName:@"input_lock" reason:[NSString stringWithFormat:@"cannot open Computer Use input ownership lock: %s",strerror(errno)] userInfo:nil];
  struct stat st;
  if(fd<0 || fstat(fd,&st)!=0 || !S_ISREG(st.st_mode) || st.st_uid!=getuid() || flock(fd,LOCK_EX|LOCK_NB)!=0) {
    if(fd>=0) close(fd);
    @throw [NSException exceptionWithName:@"input_busy" reason:@"another Computer Use session owns held input; release its key or pointer before sending input" userInfo:nil];
  }
}
static void cuRequireForeground(NSRunningApplication *expected) {
  NSRunningApplication *actual=NSWorkspace.sharedWorkspace.frontmostApplication;
  if(actual.processIdentifier!=expected.processIdentifier)
    @throw [NSException exceptionWithName:@"focus" reason:[NSString stringWithFormat:@"foreground changed to %@ (pid %d); expected %@ (pid %d). No key-down or text was sent to the new foreground application.",actual.localizedName?:@"unknown application",actual.processIdentifier,expected.localizedName?:@"bound application",expected.processIdentifier] userInfo:nil];
}
static id cuPostKey(NSDictionary *args, pid_t destination) {
  CGEventRef event=CGEventCreateKeyboardEvent(NULL,[args[@"code"] unsignedShortValue],[args[@"down"] boolValue]);
  CGEventSetFlags(event,[args[@"flags"] unsignedLongLongValue]);
  if([args[@"foreground_input"] boolValue]) CGEventPost(kCGHIDEventTap,event);
  else CGEventPostToPid(destination,event);
  CFRelease(event);
  return @{@"action_sent":@YES};
}
static void cuPrint(id result) {
  NSData *data=[NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingFragmentsAllowed error:nil];
  puts([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String); fflush(stdout);
}
static void cuReleaseLease(void) {
#ifdef CU_TEST
  if(cuTestReleaseFile) { [@"released" writeToFile:cuTestReleaseFile atomically:YES encoding:NSUTF8StringEncoding error:nil]; cuTestReleaseFile=nil; }
#endif
  if(cuLeaseKey) {
    NSMutableDictionary *up=[cuLeaseKey mutableCopy]; up[@"down"]=@NO;
    if([up[@"foreground_input"] boolValue] || !cuLeaseApp.terminated) cuPostKey(up,cuLeasePid);
    cuLeaseKey=nil; cuLeaseApp=nil;
  }
  for(int button=0;button<3;button++) if(cuLeaseButtons[button]) {
    CGEventType up=button==0?kCGEventLeftMouseUp:button==1?kCGEventRightMouseUp:kCGEventOtherMouseUp;
    CGEventRef event=CGEventCreateMouseEvent(NULL,up,cuLeasePoint,button);
    CGEventPost(kCGHIDEventTap,event); CFRelease(event); cuLeaseButtons[button]=NO;
  }
}
static void cuWaitForLease(void) {
  NSMutableData *buffer=[NSMutableData data];
  @try {
    while(!cuCancelled) {
      struct pollfd fd={STDIN_FILENO,POLLIN|POLLHUP,0};
      int ready=poll(&fd,1,100);
      if(ready<=0) continue;
      char byte; ssize_t n=read(STDIN_FILENO,&byte,1);
      if(n<=0) break;
      if(byte!='\n') { if(buffer.length>=4096) break; [buffer appendBytes:&byte length:1]; continue; }
      NSDictionary *message=[NSJSONSerialization JSONObjectWithData:buffer options:0 error:nil];
      [buffer setLength:0];
      if(![message isKindOfClass:NSDictionary.class]) break;
      NSDictionary *point=message[@"point"];
      if([point[@"x"] isKindOfClass:NSNumber.class] && [point[@"y"] isKindOfClass:NSNumber.class]) {
        cuLeasePoint=CGPointMake([point[@"x"] doubleValue],[point[@"y"] doubleValue]);
      }
      if([message[@"release"] boolValue]) break;
      cuCheckCancelled();
      if(!cuLeaseButtons[0] || !point) break;
      cuRequireForeground(cuLeaseApp);
      CGEventSourceRef source=CGEventSourceCreate(kCGEventSourceStateHIDSystemState);
      CGEventRef event=CGEventCreateMouseEvent(source,kCGEventLeftMouseDragged,cuLeasePoint,kCGMouseButtonLeft);
      CGEventSetIntegerValueField(event,kCGMouseEventClickState,1);
      CGEventPost(kCGHIDEventTap,event); CFRelease(event); CFRelease(source);
      cuPrint(@{@"action_sent":@YES,@"restored":@NO});
    }
  } @finally { cuReleaseLease(); }
}

static id attr(AXUIElementRef el, NSString *name) {
#ifdef CU_TEST
  // The observation fixture exercises the real walker without reading a GUI.
  if([(__bridge id)el isKindOfClass:NSDictionary.class]) return ((__bridge NSDictionary *)el)[name];
#endif
  CFTypeRef out = NULL;
  AXError e = AXUIElementCopyAttributeValue(el, (__bridge CFStringRef)name, &out);
  return e == kAXErrorSuccess ? CFBridgingRelease(out) : nil;
}
static NSDictionary *geometry(id v, BOOL size) {
  if (!v || CFGetTypeID((__bridge CFTypeRef)v) != AXValueGetTypeID()) return nil;
  if (size) { CGSize s; if (AXValueGetValue((__bridge AXValueRef)v,kAXValueCGSizeType,&s)) return @{ @"w":@(s.width), @"h":@(s.height) }; }
  else { CGPoint p; if (AXValueGetValue((__bridge AXValueRef)v,kAXValueCGPointType,&p)) return @{ @"x":@(p.x), @"y":@(p.y) }; }
  return nil;
}
static NSDictionary *info(AXUIElementRef el, NSInteger index, NSInteger win, NSArray *path) {
  NSMutableDictionary *d = [@{@"index":@(index), @"windowIndex":@(win), @"path":path} mutableCopy];
  for (NSString *key in @[@"role",@"subrole",@"value",@"enabled",@"focused"]) {
    NSDictionary *names = @{@"role":@"AXRole",@"subrole":@"AXSubrole",@"value":@"AXValue",@"enabled":@"AXEnabled",@"focused":@"AXFocused"};
    id v = attr(el,names[key]);
    if ([v isKindOfClass:NSString.class]) d[key] = [v length]>12000 ? [v substringToIndex:12000] : v;
    else if ([v isKindOfClass:NSNumber.class]) d[key] = v;
  }
  id label = attr(el,@"AXTitle");
  if (![label isKindOfClass:NSString.class] || ![label length]) label = attr(el,@"AXDescription");
  if ([label isKindOfClass:NSString.class]) d[@"label"] = label;
  id p=geometry(attr(el,@"AXPosition"),NO), s=geometry(attr(el,@"AXSize"),YES);
  if(p) d[@"position"]=p; if(s) d[@"size"]=s;
  CFArrayRef actions=NULL;
#ifdef CU_TEST
  if([(__bridge id)el isKindOfClass:NSDictionary.class]) d[@"actions"]=attr(el,@"actions")?:@[];
  else
#endif
  if(AXUIElementCopyActionNames(el,&actions)==kAXErrorSuccess) d[@"actions"]=CFBridgingRelease(actions);
  else d[@"actions"]=@[];
  return d;
}
static void cuValidateElementIdentity(AXUIElementRef el, NSDictionary *target) {
  NSDictionary *current=info(el,0,0,@[]);
  for(NSString *key in @[@"role",@"label"]) {
    id expected=target[key]?:NSNull.null, actual=current[key]?:NSNull.null;
    if(![expected isEqual:actual]) @throw [NSException exceptionWithName:@"stale" reason:[NSString stringWithFormat:@"element changed %@; observe again before acting",key] userInfo:nil];
  }
}
static void walk(AXUIElementRef el, NSInteger win, NSArray *path, NSInteger depth, NSInteger limit, NSInteger max, BOOL depthIsTruncation, NSMutableArray *out, BOOL *truncated) {
  // Breadth first keeps a long file listing from hiding its dialog buttons.
  NSMutableArray *queue=[NSMutableArray arrayWithObject:@{@"el":(__bridge id)el,@"path":path,@"depth":@(depth)}];
  for(NSUInteger cursor=0;cursor<queue.count;cursor++) {
    if(out.count>=max){ *truncated=YES; break; }
    NSDictionary *item=queue[cursor];
    AXUIElementRef current=(__bridge AXUIElementRef)item[@"el"];
    NSArray *currentPath=item[@"path"];
    NSInteger currentDepth=[item[@"depth"] integerValue];
    [out addObject:info(current,out.count,win,currentPath)];
    NSArray *kids=attr(current,@"AXChildren");
    if(currentDepth>=limit){ if(kids.count && depthIsTruncation) *truncated=YES; continue; }
    for(NSUInteger i=0;i<kids.count;i++) {
      if(queue.count-cursor>=(NSUInteger)max){ *truncated=YES; break; }
      [queue addObject:@{@"el":kids[i],@"path":[currentPath arrayByAddingObject:@(i)],@"depth":@(currentDepth+1)}];
    }
  }
}
static NSArray *observeElements(AXUIElementRef app, NSArray *ws, NSDictionary *args, BOOL listWindows, BOOL *truncated) {
  NSMutableArray *out=[NSMutableArray array];
  BOOL full=[args[@"detail"] isEqual:@"full"];
  NSInteger limit=full?16:10, max=full?800:400;
  if(!listWindows && !args[@"window_id"]) {
    // Open popup menus remain useful. Hidden menu-bar descendants belong in
    // the full view; summary reserves their budget for the app's actual UI.
    NSInteger menuMax=max/4;
    NSArray *children=attr(app,@"AXChildren");
    for(NSUInteger i=0;i<children.count;i++) {
      NSString *role=attr((__bridge AXUIElementRef)children[i],@"AXRole");
      if([role isEqual:@"AXMenu"]) walk((__bridge AXUIElementRef)children[i],-2,@[@(i)],0,limit,menuMax/2,YES,out,truncated);
    }
    id menu=attr(app,@"AXMenuBar");
    if(menu) walk((__bridge AXUIElementRef)menu,-1,@[],0,full?limit:1,menuMax,full,out,truncated);
  }
  for(NSUInteger i=0;i<ws.count;i++) {
    if(args[@"window_id"] && i!=[args[@"window_id"] unsignedIntegerValue]) continue;
    if(listWindows){ NSMutableDictionary *d=[info((__bridge AXUIElementRef)ws[i],i,i,@[]) mutableCopy]; d[@"title"]=d[@"label"]?:@""; [out addObject:d]; }
    else walk((__bridge AXUIElementRef)ws[i],i,@[],0,limit,max,YES,out,truncated);
  }
  return out;
}
static BOOL cuFrame(AXUIElementRef el, CGRect *out) {
  NSDictionary *p=geometry(attr(el,@"AXPosition"),NO), *z=geometry(attr(el,@"AXSize"),YES);
  if(!p || !z) return NO;
  *out=CGRectMake([p[@"x"] doubleValue],[p[@"y"] doubleValue],[z[@"w"] doubleValue],[z[@"h"] doubleValue]);
  return YES;
}
static NSDictionary *capturableWindow(NSArray *windows, pid_t pid, NSString *name, CGRect preferred) {
  NSDictionary *matched=nil;
  for(NSDictionary *w in windows) {
    if([w[(__bridge NSString *)kCGWindowOwnerPID] intValue]!=pid || [w[(__bridge NSString *)kCGWindowLayer] intValue]!=0) continue;
    CGRect b; if(!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)w[(__bridge NSString *)kCGWindowBounds],&b) || b.size.width<1 || b.size.height<1) continue;
    if(fabs(b.origin.x-preferred.origin.x)>1 || fabs(b.origin.y-preferred.origin.y)>1 || fabs(b.size.width-preferred.size.width)>1 || fabs(b.size.height-preferred.size.height)>1) continue;
    if(matched) @throw [NSException exceptionWithName:@"window" reason:@"the selected window is ambiguous; observe the app windows again" userInfo:nil];
    matched=@{@"window_id":w[(__bridge NSString *)kCGWindowNumber],@"name":name?:@"App",@"points":@{@"x":@(b.origin.x),@"y":@(b.origin.y),@"w":@(b.size.width),@"h":@(b.size.height)}};
  }
  if(!matched) @throw [NSException exceptionWithName:@"window" reason:@"the selected app window is not capturable; observe the app windows again" userInfo:nil];
  return matched;
}
static NSArray *cuActions(AXUIElementRef el) {
  CFArrayRef names=NULL;
#ifdef CU_TEST
  if([(__bridge id)el isKindOfClass:NSDictionary.class]) return attr(el,@"actions")?:@[];
#endif
  return AXUIElementCopyActionNames(el,&names)==kAXErrorSuccess?CFBridgingRelease(names):@[];
}
static BOOL cuSettable(AXUIElementRef el, NSString *name) {
#ifdef CU_TEST
  if([(__bridge id)el isKindOfClass:NSDictionary.class]) return [attr(el,@"settable") containsObject:name];
#endif
  Boolean settable=false;
  return AXUIElementIsAttributeSettable(el,(__bridge CFStringRef)name,&settable)==kAXErrorSuccess && settable;
}
static NSString *cuClickAction(AXUIElementRef el, BOOL context) {
  id enabled=attr(el,@"AXEnabled");
  if([enabled isKindOfClass:NSNumber.class] && ![enabled boolValue]) return nil;
  NSArray *actions=cuActions(el);
  if(context) return [actions containsObject:@"AXShowMenu"]?@"AXShowMenu":nil;
  NSString *role=attr(el,@"AXRole");
  // Pressing a text field is toolkit-dependent; focus its insertion point directly.
  if([@[@"AXTextField",@"AXTextArea",@"AXComboBox"] containsObject:role] && cuSettable(el,@"AXFocused")) return @"AXFocused";
  if([actions containsObject:@"AXPress"]) return @"AXPress";
  if([role isEqual:@"AXMenuItem"] && [actions containsObject:@"AXPick"]) return @"AXPick";
  if([@[@"AXRow",@"AXCell"] containsObject:role] && cuSettable(el,@"AXSelected")) return @"AXSelected";
  return nil;
}
static NSDictionary *cuClick(AXUIElementRef el, BOOL context) {
  NSString *action=cuClickAction(el,context);
  if(!action) @throw [NSException exceptionWithName:@"background_action_unavailable" reason:@"this control has no supported accessibility click; observe its advertised actions or use a separate computer" userInfo:nil];
  cuCheckCancelled();
  BOOL attribute=[action isEqual:@"AXFocused"] || [action isEqual:@"AXSelected"];
  AXError error=attribute?AXUIElementSetAttributeValue(el,(__bridge CFStringRef)action,kCFBooleanTrue):AXUIElementPerformAction(el,(__bridge CFStringRef)action);
  if(error!=kAXErrorSuccess) @throw [NSException exceptionWithName:@"action" reason:[NSString stringWithFormat:@"accessibility %@ failed: %d; no pointer fallback was sent",action,error] userInfo:nil];
  return @{@"action_sent":@YES,@"strategy":@"a11y",@"action":action,@"pointer_moved":@NO,
           @"verified":@(attribute && [attr(el,action) boolValue])};
}
static id cuScrollBar(AXUIElementRef el, BOOL horizontal) {
  id enabled=attr(el,@"AXEnabled");
  if([enabled isKindOfClass:NSNumber.class] && ![enabled boolValue]) return nil;
  return attr(el,horizontal?@"AXHorizontalScrollBar":@"AXVerticalScrollBar");
}
static NSDictionary *cuScroll(AXUIElementRef el, NSDictionary *args) {
  BOOL horizontal=[@[@"left",@"right"] containsObject:args[@"direction"]];
  id bar=nil;
  for(id cur=(__bridge id)el;cur && !bar;) {
    AXUIElementRef node=(__bridge AXUIElementRef)cur;
    bar=cuScrollBar(node,horizontal);
    if([attr(node,@"AXRole") isEqual:@"AXWindow"]) break;
    cur=attr(node,@"AXParent");
  }
  if(!bar) @throw [NSException exceptionWithName:@"background_scroll_unavailable" reason:@"no accessibility scrollbar at this target; choose an observed scroll area or a separate computer" userInfo:nil];
  AXUIElementRef control=(__bridge AXUIElementRef)bar;
  BOOL forward=[@[@"down",@"right"] containsObject:args[@"direction"]];
  NSString *action=forward?@"AXIncrement":@"AXDecrement";
  NSInteger count=MAX(1,MIN(100,[args[@"amount"] integerValue]));
  id before=attr(control,@"AXValue");
  BOOL advertised=[cuActions(control) containsObject:action];
  // Native scrollbars commonly expose a normalized value instead of actions.
  // Report that unit explicitly: it is not a claim about a toolkit's line size.
  BOOL normalized=!advertised && [before isKindOfClass:NSNumber.class] && [before doubleValue]>=0 && [before doubleValue]<=1 && cuSettable(control,@"AXValue");
  if(!advertised && !normalized) @throw [NSException exceptionWithName:@"background_scroll_unavailable" reason:@"the accessibility scrollbar has no supported action or writable normalized value" userInfo:nil];
  for(NSInteger i=0;i<(advertised?count:1);i++) {
    cuCheckCancelled();
    NSNumber *value=@(MAX(0,MIN(1,[before doubleValue]+(forward?1:-1)*0.05*count)));
    AXError error=advertised?AXUIElementPerformAction(control,(__bridge CFStringRef)action):AXUIElementSetAttributeValue(control,kAXValueAttribute,(__bridge CFTypeRef)value);
    if(error!=kAXErrorSuccess) @throw [NSException exceptionWithName:@"action" reason:[NSString stringWithFormat:@"accessibility scroll failed: %d; no pointer fallback was sent",error] userInfo:nil];
  }
  id after=attr(control,@"AXValue");
  return @{@"action_sent":@YES,@"strategy":@"a11y",@"pointer_moved":@NO,@"action":advertised?action:@"AXValue",
           @"unit":advertised?@"accessibility_increment":@"normalized_scrollbar",@"before":before?:NSNull.null,@"after":after?:NSNull.null,
           @"verified":@(before && after && ![before isEqual:after])};
}
/**
 * Smallest pressable element whose frame contains p.
 *
 * AXUIElementCopyElementAtPosition is the first resolver, but several toolkits
 * (Chromium's browser process among them) answer it with the window rather
 * than the control the user sees, so a coordinate would silently degrade to a
 * raw event. Searching the subtree geometrically recovers the real target;
 * "smallest containing" is what picks the button instead of its group. Bounded
 * so a huge tree cannot stall an action.
 */
static void cuSearch(AXUIElementRef el, CGPoint p, int depth, int *budget, id *best, double *bestArea, NSString *operation) {
  if(depth>24 || (*budget)--<=0) return;
  CGRect frame;
  if(cuFrame(el,&frame)) {
    // Children are laid out inside their parent (and clipped when they are
    // not), so a frame that misses the point prunes the whole subtree.
    if(!CGRectContainsPoint(frame,p)) return;
    double area=frame.size.width*frame.size.height;
    BOOL suitable=[operation hasPrefix:@"scroll"]?cuScrollBar(el,[operation isEqual:@"scroll-horizontal"])!=nil:cuClickAction(el,[operation isEqual:@"context"])!=nil;
    if(suitable && (!*best || area<=*bestArea)) { *best=(__bridge id)el; *bestArea=area; }
  }
  for(id kid in attr(el,@"AXChildren")) cuSearch((__bridge AXUIElementRef)kid,p,depth+1,budget,best,bestArea,operation);
}
/**
 * Every key an app_ref supplies must match. Matching any one of them would let
 * {pid, bundle_id} land on a *different* process of the same bundle — the
 * user's own browser instead of the one the agent opened — and then type into
 * their window. Identity here is a conjunction, deliberately.
 */
static BOOL matchesName(NSString *have, NSString *want) {
  return have && [have caseInsensitiveCompare:want]==NSOrderedSame;
}
/**
 * Bring an application forward. -[NSRunningApplication activateWithOptions:]
 * is ignored on macOS 14+ when the caller is not itself frontmost, which a
 * background helper never is; setting AXFrontmost goes through the
 * Accessibility grant this process actually holds.
 */
static BOOL axActivate(pid_t pid) {
  AXUIElementRef app=AXUIElementCreateApplication(pid);
  AXError e=AXUIElementSetAttributeValue(app,kAXFrontmostAttribute,kCFBooleanTrue);
  CFRelease(app);
  return e==kAXErrorSuccess;
}
static NSRunningApplication *resolve(NSDictionary *ref) {
  // Only omission selects the frontmost app. An explicit but malformed
  // identity must never redirect observation or input to the user's app.
  if(!ref) return NSWorkspace.sharedWorkspace.frontmostApplication;
  if(![ref isKindOfClass:NSDictionary.class] || !ref.count) return nil;
  for(id key in ref) {
    id value=ref[key];
    if([key isEqual:@"pid"]) {
      if(![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value)==CFBooleanGetTypeID()
         || [value doubleValue]<=0 || [value doubleValue]>INT_MAX || [value doubleValue]!=[value intValue]) return nil;
    } else if([key isEqual:@"name"] || [key isEqual:@"bundle_id"]) {
      if(![value isKindOfClass:NSString.class] || ![value stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet].length) return nil;
    } else return nil;
  }
  NSString *bundle=ref[@"bundle_id"], *name=ref[@"name"];
  for(NSRunningApplication *a in NSWorkspace.sharedWorkspace.runningApplications) {
    if(ref[@"pid"] && a.processIdentifier!=[ref[@"pid"] intValue]) continue;
    if(bundle && !matchesName(a.bundleIdentifier,bundle)) continue;
    if(name && !matchesName(a.localizedName,name)) continue;
    return a;
  }
  return nil;
}
static CGEventRef textEvent(NSString *text, BOOL down) {
  UniChar *chars=calloc(text.length,sizeof(UniChar)); [text getCharacters:chars range:NSMakeRange(0,text.length)];
  CGEventRef event=CGEventCreateKeyboardEvent(NULL,0,down);
#ifdef CU_TEST
  // Simulate physical modifier state without posting a system key event.
  CGEventSetFlags(event,cuTestInheritedTextFlags);
#endif
  // Literal text must not inherit the user's held Command/Control/Option/Shift.
  CGEventSetFlags(event,0);
  CGEventKeyboardSetUnicodeString(event,text.length,chars); free(chars); return event;
}
static BOOL cuTextRole(NSString *role) {
  return [@[@"AXTextField",@"AXTextArea",@"AXComboBox",@"AXSearchField",@"AXSecureTextField",@"AXWebArea"] containsObject:role];
}
// Without a readable selection range only an exact append can be verified.
// Matching length or an already-present suffix is not evidence of delivery.
static BOOL cuTypeVerified(NSString *before, NSString *after, NSString *text) {
  return before && after && [after isEqual:[before stringByAppendingString:text]];
}
static id cuFocusedElement(pid_t pid) {
  AXUIElementRef appEl=AXUIElementCreateApplication(pid);
  AXUIElementSetMessagingTimeout(appEl,2.0);
  id focused=attr(appEl,@"AXFocusedUIElement");
  CFRelease(appEl);
  return focused;
}
/**
 * Type into whatever holds focus in the bound app, then prove it landed.
 * Dispatch succeeding is not delivery (a process with no text receiver drops
 * the events silently), so the receipt reports `verified` from the focused
 * control's own value. Failure to verify is reported, not thrown — the events
 * already went out. The one throw is before any event is posted: a focused
 * element that is clearly not a text control.
 */
static NSDictionary *cuType(NSDictionary *args, NSRunningApplication *inputApp, id focused, BOOL simulated) {
  NSString *text=args[@"text"];
  if(![text isKindOfClass:NSString.class]) @throw [NSException exceptionWithName:@"text" reason:@"text must be a string" userInfo:nil];
  NSString *role=focused?attr((__bridge AXUIElementRef)focused,@"AXRole"):nil;
  BOOL secure=[role isEqual:@"AXSecureTextField"];
  NSString *before=nil;
  // A secure field's value is never read; it verifies as unverifiable.
  if(focused && !secure) { id v=attr((__bridge AXUIElementRef)focused,@"AXValue"); if([v isKindOfClass:NSString.class]) before=v; }
  // Fail closed only on strong evidence: something holds focus and it is
  // clearly not text. No focused element at all still receives the events —
  // some apps take process-directed keys without reporting AX focus.
  if(focused && !cuTextRole(role) && !before)
    @throw [NSException exceptionWithName:@"focus" reason:[NSString stringWithFormat:@"focused element is a %@, not a text control — click or focus a text field first",role?:@"unknown element"] userInfo:nil];
  NSString *expected=nil;
  if(before && focused) {
    id range=attr((__bridge AXUIElementRef)focused,@"AXSelectedTextRange"); CFRange selected;
    if(range && CFGetTypeID((__bridge CFTypeRef)range)==AXValueGetTypeID() && AXValueGetValue((__bridge AXValueRef)range,kAXValueCFRangeType,&selected)
       && selected.location>=0 && selected.length>=0 && selected.location<=before.length && selected.length<=before.length-selected.location)
      expected=[before stringByReplacingCharactersInRange:NSMakeRange(selected.location,selected.length) withString:text];
  }
  BOOL semantic=!simulated && focused && ![args[@"foreground_input"] boolValue] && cuSettable((__bridge AXUIElementRef)focused,@"AXSelectedText");
  if(semantic) {
    cuCheckCancelled();
    AXError error=AXUIElementSetAttributeValue((__bridge AXUIElementRef)focused,kAXSelectedTextAttribute,(__bridge CFStringRef)text);
    if(error!=kAXErrorSuccess) @throw [NSException exceptionWithName:@"action" reason:[NSString stringWithFormat:@"accessibility text insertion failed: %d; observe before retrying; no keyboard fallback was sent",error] userInfo:nil];
  }
  // One grapheme per event, the way a keyboard delivers them. Batching
  // several into one CGEventKeyboardSetUnicodeString is faster but Electron
  // apps coalesce the pending payload and keep only the final batch, so a
  // typed string silently arrives truncated to its tail.
  for(NSUInteger i=0;!semantic && i<text.length && !cuCancelled;) {
    if([args[@"foreground_input"] boolValue]) cuRequireForeground(inputApp);
    cuCheckCancelled();
    NSRange range=[text rangeOfComposedCharacterSequencesForRange:NSMakeRange(i,1)];
    NSString *chunk=[text substringWithRange:range];
    if(!simulated) for(int down=1;down>=0;down--){ CGEventRef event=textEvent(chunk,down); if([args[@"foreground_input"] boolValue]) CGEventPost(kCGHIDEventTap,event); else CGEventPostToPid(inputApp.processIdentifier,event); CFRelease(event); }
    i=NSMaxRange(range); usleep(10000);
  }
  if(cuCancelled) @throw [NSException exceptionWithName:@"cancelled" reason:@"computer request cancelled" userInfo:nil];
  NSString *after=nil;
  if(focused && !secure) {
#ifdef CU_TEST
    if(simulated) { NSString *s=((NSMutableDictionary *)focused)[@"after"]; if(s) ((NSMutableDictionary *)focused)[@"AXValue"]=s; }
    else
#endif
    usleep(80000);
    id v=attr((__bridge AXUIElementRef)focused,@"AXValue");
    if([v isKindOfClass:NSString.class]) after=v;
  }
  BOOL verified=expected?[after isEqual:expected]:cuTypeVerified(before,after,text);
  NSMutableDictionary *receipt=[@{@"action_sent":@YES,@"chars":@(text.length),@"strategy":semantic?@"a11y-selected-text":@"unicode-events",
                                  @"keyboard_delivery":semantic?@"accessibility":[args[@"foreground_input"] boolValue]?@"foreground-guarded":@"process",
                                  @"verified":@(verified),@"focused_role":role?:[NSNull null]} mutableCopy];
  if(!verified) receipt[@"verification_required"]=@"screenshot";
  return receipt;
}
static NSDictionary *windowAtPoint(NSArray *windows, CGPoint p) {
    NSMutableArray *skipped=[NSMutableArray array];
    for(NSDictionary *w in windows) {          // front to back
      CGRect b;
      if(!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)w[(__bridge NSString *)kCGWindowBounds],&b)) continue;
      if(!CGRectContainsPoint(b,p)) continue;
      pid_t owner=[w[(__bridge NSString *)kCGWindowOwnerPID] intValue];
      NSString *name=w[(__bridge NSString *)kCGWindowOwnerName]?:@"";
      NSNumber *alpha=w[(__bridge NSString *)kCGWindowAlpha], *layer=w[(__bridge NSString *)kCGWindowLayer]?:@0;
      // Visible floating windows occlude input just like normal windows.
      if(alpha && [alpha doubleValue]<=0) { [skipped addObject:@{@"owner":name,@"why":@"transparent"}]; continue; }
      return @{@"found":@YES,@"owner_pid":@(owner),@"owner_name":name,
               @"window_id":w[(__bridge NSString *)kCGWindowNumber]?:@0,@"layer":layer,
               @"skipped":skipped};
    }
    return @{@"found":@NO,@"skipped":skipped};
}

static id execute(NSDictionary *p) {
  NSString *tool=p[@"tool"]; NSDictionary *args=p[@"args"]?:@{};
  if([tool isEqual:@"pointer_sequence"] && ![args[@"foreground_input"] boolValue])
    @throw [NSException exceptionWithName:@"shared_pointer_required" reason:@"shared macOS pointer input is unavailable in background mode; use an accessibility action or a separate computer" userInfo:nil];
  cuOwnerPipe=[args[@"owner_pipe"] boolValue];
  BOOL mutates=[@[@"type",@"key_event",@"mouse_event",@"scroll",@"pointer_sequence",@"release_input",@"set_value",@"select_text",@"perform_action",@"click_element",@"scroll_element"] containsObject:tool]
    || ([tool isEqual:@"hit_test"] && [args[@"perform"] boolValue])
    || ([tool isEqual:@"app_info"] && [args[@"activate"] boolValue]);
  if([tool isEqual:@"release_input"]) {
    if(!AXIsProcessTrusted()) @throw [NSException exceptionWithName:@"permission" reason:@"Accessibility permission is missing" userInfo:nil];
    cuLockInput();
    NSDictionary *point=args[@"point"];
    CGPoint at=CGPointMake([point[@"x"] doubleValue],[point[@"y"] doubleValue]);
    CGMouseButton button=[args[@"button"] unsignedIntValue];
    CGEventType up=button==0?kCGEventLeftMouseUp:button==1?kCGEventRightMouseUp:kCGEventOtherMouseUp;
    CGEventRef event=CGEventCreateMouseEvent(NULL,up,at,button);
    CGEventPost(kCGHIDEventTap,event); CFRelease(event);
    return @{@"released":@YES};
  }
  if([tool isEqual:@"key_event"] && ![args[@"down"] boolValue] && [args[@"owned_release"] boolValue]) {
    if(!AXIsProcessTrusted()) @throw [NSException exceptionWithName:@"permission" reason:@"Accessibility permission is missing" userInfo:nil];
    cuLockInput();
    // Release a confirmed/ambiguous press even if its original app has exited.
    return cuPostKey(args,[args[@"input_app_ref"][@"pid"] intValue]);
  }
  cuCheckCancelled();
  if([tool isEqual:@"input_capabilities"]) return @{@"input_lease":@1,@"owner_pipe":@YES,@"record_owner_pipe":@1,@"window_ocr":@1,@"element_identity":@1,@"background_actions":@1};
  if([tool isEqual:@"record"]) return cuRecord(args);
  if([tool isEqual:@"recognize_text"]) return cuRecognizeText(args[@"file"]);
#ifdef CU_TEST
  if([tool isEqual:@"inspect_click_action"]) return @{@"action":cuClickAction((__bridge AXUIElementRef)args[@"element"],[args[@"context"] boolValue])?:NSNull.null};
  if([tool isEqual:@"inspect_element_identity"]) {
    cuValidateElementIdentity((__bridge AXUIElementRef)args[@"element"],args[@"target"]);
    return @{@"identity_matches":@YES};
  }
  if([tool isEqual:@"inspect_window_at_point"]) return windowAtPoint(args[@"windows"],CGPointMake([args[@"x"] doubleValue],[args[@"y"] doubleValue]));
  if([tool isEqual:@"inspect_window_match"]) {
    NSDictionary *b=args[@"bounds"];
    CGRect bounds=CGRectMake([b[@"x"] doubleValue],[b[@"y"] doubleValue],[b[@"w"] doubleValue],[b[@"h"] doubleValue]);
    return capturableWindow(args[@"windows"],[args[@"pid"] intValue],@"Fixture",bounds);
  }
  if([tool isEqual:@"inspect_observation"]) {
    BOOL truncated=NO;
    NSArray *elements=observeElements((__bridge AXUIElementRef)args[@"app"],args[@"windows"]?:@[],args,NO,&truncated);
    return @{@"elements":elements,@"truncated":@(truncated)};
  }
  if([tool isEqual:@"test_input_lease"]) {
    cuTestLockDir=args[@"lock_dir"]; cuLockInput();
    cuTestReleaseFile=args[@"release_file"];
    if([args[@"work_ms"] intValue]>0) {
      cuPrint(@{@"action_sent":@YES,@"input_lease":@YES});
      for(int elapsed=0;elapsed<[args[@"work_ms"] intValue];elapsed+=20) { cuCheckCancelled(); usleep(20000); }
    }
    return @{@"action_sent":@YES};
  }
  if([tool isEqual:@"inspect_text_event"]) {
    cuTestInheritedTextFlags=[args[@"inherited_flags"] unsignedLongLongValue];
    CGEventRef event=textEvent(args[@"text"],YES); UniChar chars[4096]; UniCharCount length=0;
    CGEventKeyboardGetUnicodeString(event,4096,&length,chars); CGEventFlags flags=CGEventGetFlags(event); CFRelease(event);
    return @{@"text":[NSString stringWithCharacters:chars length:length],@"flags":@(flags)};
  }
  // Drives the real typing logic against a fixture focused element instead of
  // a live app: `after` is the value the element reports once the text lands,
  // which a fixture omits to model an app that swallows the events.
  if([tool isEqual:@"inspect_type"]) {
    id fixture=args[@"focused"];
    return cuType(args, nil, [fixture isKindOfClass:NSDictionary.class]?[fixture mutableCopy]:nil, YES);
  }
#endif
  if([tool isEqual:@"permissions"]) return @{@"trusted":@(AXIsProcessTrusted())};
  if([tool isEqual:@"list_apps"]) {
    NSMutableArray *apps=[NSMutableArray array];
    for(NSRunningApplication *a in NSWorkspace.sharedWorkspace.runningApplications)
      [apps addObject:@{@"name":a.localizedName?:@"",@"pid":@(a.processIdentifier),@"bundle_id":a.bundleIdentifier?:@"",@"frontmost":@(a.active),@"hidden":@(a.hidden)}];
    return @{@"apps":apps};
  }
  if([tool isEqual:@"displays"]) {
    uint32_t n=0; CGGetActiveDisplayList(0,NULL,&n); CGDirectDisplayID ids[n]; CGGetActiveDisplayList(n,ids,&n);
    NSMutableArray *out=[NSMutableArray array];
    for(uint32_t i=0;i<n;i++){ CGRect b=CGDisplayBounds(ids[i]); CGDisplayModeRef mode=CGDisplayCopyDisplayMode(ids[i]);
      size_t w=CGDisplayModeGetPixelWidth(mode),h=CGDisplayModeGetPixelHeight(mode); CGDisplayModeRelease(mode);
      [out addObject:@{@"index":@(i+1),@"id":@(ids[i]),@"main":@(ids[i]==CGMainDisplayID()),@"points":@{@"x":@(b.origin.x),@"y":@(b.origin.y),@"w":@(b.size.width),@"h":@(b.size.height)},@"pixels":@{@"w":@(w),@"h":@(h)},@"scale":@(w/b.size.width)}]; }
    return out;
  }
  if([tool isEqual:@"preview_notify"]) {
    [[NSDistributedNotificationCenter defaultCenter] postNotificationName:@"net.codewhale.computer-use.preview" object:nil userInfo:args deliverImmediately:YES];
    return @{@"updated":@YES};
  }
  if([tool isEqual:@"window_info"]) {
    NSRunningApplication *a=resolve(args[@"app_ref"]?:args[@"input_app_ref"]);
    if(!a) @throw [NSException exceptionWithName:@"app" reason:@"application not found" userInfo:nil];
    AXUIElementRef ax=AXUIElementCreateApplication(a.processIdentifier);
    NSArray *axWindows=attr(ax,@"AXWindows");
    NSInteger index=[args[@"window_id"] integerValue];
    CGRect preferred;
    BOOL integerIndex=!args[@"window_id"] || ([args[@"window_id"] isKindOfClass:NSNumber.class] && [args[@"window_id"] doubleValue]==index);
    BOOL valid=integerIndex && index>=0 && index<axWindows.count && cuFrame((__bridge AXUIElementRef)axWindows[index],&preferred);
    CFRelease(ax);
    if(!valid) @throw [NSException exceptionWithName:@"window" reason:@"the selected app window has no accessibility geometry; call list_windows for a valid window index" userInfo:nil];
    NSArray *windows=CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionAll,kCGNullWindowID));
    return capturableWindow(windows,a.processIdentifier,a.localizedName,preferred);
  }
  // Which application owns the point a pointer event would land on. A global
  // pointer event goes to whatever is on top, so this is what stops a click
  // meant for the agent's app from landing in the user's window.
  if([tool isEqual:@"window_at_point"]) {
    CGPoint p=CGPointMake([args[@"x"] doubleValue],[args[@"y"] doubleValue]);
    NSArray *windows=CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly|kCGWindowListExcludeDesktopElements,kCGNullWindowID));
    return windowAtPoint(windows,p);
  }
  if([tool isEqual:@"app_info"]) {
    NSRunningApplication *a=resolve(args[@"app_ref"]);
    if(!a) @throw [NSException exceptionWithName:@"app" reason:@"application not found" userInfo:nil];
    if([args[@"activate"] boolValue]) cuLockInput();
    cuCheckCancelled();
    if([args[@"activate"] boolValue] && !axActivate(a.processIdentifier)) [a activateWithOptions:0];
    if([args[@"activate"] boolValue]) for(int i=0;i<120;i++) {
      cuCheckCancelled();
      if(NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier==a.processIdentifier) break;
      usleep(25000);
    }
    return @{@"found":@YES,@"name":a.localizedName?:@"",@"pid":@(a.processIdentifier),@"bundle_id":a.bundleIdentifier?:@"",@"frontmost":@(a.active)};
  }
  NSRunningApplication *inputApp=nil;
  if([@[@"type",@"key_event",@"mouse_event",@"scroll",@"hit_test",@"pointer_sequence"] containsObject:tool]) {
    if(![args[@"input_app_ref"] isKindOfClass:NSDictionary.class]) @throw [NSException exceptionWithName:@"focus" reason:@"open_application first to bind the input destination" userInfo:nil];
    inputApp=resolve(args[@"input_app_ref"]);
    if(!inputApp || inputApp.terminated) @throw [NSException exceptionWithName:@"focus" reason:@"input application is no longer running; open_application again" userInfo:nil];
  }
  if(mutates) { cuCheckCancelled(); cuLockInput(); }
  if(!AXIsProcessTrusted()) @throw [NSException exceptionWithName:@"permission" reason:@"Accessibility permission is missing for Codewhale Computer Use (or the direct host)." userInfo:nil];
  if([tool isEqual:@"type"]) return cuType(args, inputApp, cuFocusedElement(inputApp.processIdentifier), NO);
  if([tool isEqual:@"key_event"]) {
    if([args[@"foreground_input"] boolValue] && [args[@"down"] boolValue]) cuRequireForeground(inputApp);
    cuCheckCancelled();
    id result=cuPostKey(args,inputApp.processIdentifier);
    if([args[@"input_lease"] boolValue] && [args[@"down"] boolValue]) { cuLeaseKey=args; cuLeasePid=inputApp.processIdentifier; cuLeaseApp=inputApp; }
    return result;
  }
  if([tool isEqual:@"mouse_event"]) {
    CGPoint p=CGPointMake([args[@"x"] doubleValue],[args[@"y"] doubleValue]);
    CGEventRef event=CGEventCreateMouseEvent(NULL,[args[@"type"] unsignedIntValue],p,[args[@"button"] unsignedIntValue]);
    CGEventSetIntegerValueField(event,kCGMouseEventClickState,[args[@"clickState"] longLongValue]);
    // Address the window as well as the process using public event fields.
    // Dispatch is not delivery: a toolkit may still discard these events.
    // This primitive needs effect readback before a caller can rely on it.
    if([args[@"windowNumber"] longLongValue]>0) {
      CGEventSetIntegerValueField(event,kCGMouseEventWindowUnderMousePointer,[args[@"windowNumber"] longLongValue]);
      CGEventSetIntegerValueField(event,kCGMouseEventWindowUnderMousePointerThatCanHandleThisEvent,[args[@"windowNumber"] longLongValue]);
    }
    cuCheckCancelled();
    CGEventPostToPid(inputApp.processIdentifier,event); CFRelease(event); return @{@"action_sent":@YES};
  }
  // Accessibility-first coordinate action: resolve the point against the bound
  // application's AX tree and press the element it names. Callers fall back to
  // raw CGEvents when this reports found=NO, so it must fail closed rather than
  // guess: a point owned by another process, or a point that only lands on a
  // container, is not a press.
  if([tool isEqual:@"hit_test"]) {
    CGPoint p=CGPointMake([args[@"x"] doubleValue],[args[@"y"] doubleValue]);
    AXUIElementRef appEl=AXUIElementCreateApplication(inputApp.processIdentifier);
    AXUIElementSetMessagingTimeout(appEl,2.0);
    AXUIElementRef raw=NULL;
    AXError err=AXUIElementCopyElementAtPosition(appEl,(float)p.x,(float)p.y,&raw);
    id hit=nil;
    if(err==kAXErrorSuccess && raw) {
      pid_t owner=0;
      if(AXUIElementGetPid(raw,&owner)==kAXErrorSuccess && owner==inputApp.processIdentifier) hit=CFBridgingRelease(raw);
      else CFRelease(raw); // Another app may cover a background window. Search only our own tree below.
    }

    id chosen=nil;
    BOOL insideSheet=NO;
    NSString *operation=args[@"operation"]?:@"click";
    BOOL scrolling=[operation hasPrefix:@"scroll"];
    // 1. The element under the point, or the nearest ancestor that can be
    //    pressed — a label inside a button is the common case.
    for(id cur=hit; cur && !chosen;) {
      AXUIElementRef el=(__bridge AXUIElementRef)cur;
      id role=attr(el,@"AXRole");
      if([role isEqual:@"AXSheet"]) insideSheet=YES;
      if([role isEqual:@"AXWindow"] || [role isEqual:@"AXApplication"]) break;
      if(scrolling?cuScrollBar(el,[operation isEqual:@"scroll-horizontal"])!=nil:cuClickAction(el,[operation isEqual:@"context"])!=nil) { chosen=cur; break; }
      cur=attr(el,@"AXParent");
    }
    // 2. Otherwise search downward for the smallest control covering the point.
    if(!chosen) {
      int budget=1500; double area=0; id best=nil;
      if(hit) cuSearch((__bridge AXUIElementRef)hit,p,0,&budget,&best,&area,operation);
      else for(id w in attr(appEl,@"AXWindows")) {
        CGRect frame;
        if(!cuFrame((__bridge AXUIElementRef)w,&frame) || !CGRectContainsPoint(frame,p)) continue;
        // A sheet owns the window's interaction, even when the sheet does not cover p.
        NSArray *sheets=attr((__bridge AXUIElementRef)w,@"AXSheets");
        if(sheets.count) {
          for(id sheet in sheets) cuSearch((__bridge AXUIElementRef)sheet,p,0,&budget,&best,&area,operation);
          insideSheet=YES;
        } else cuSearch((__bridge AXUIElementRef)w,p,0,&budget,&best,&area,operation);
        break; // Never click through another window of the same app.
      }
      chosen=best;
    }
    CFRelease(appEl);
    if(!chosen) return @{@"found":@NO,@"reason":hit?@"no_pressable_element_at_point":@"no_element_at_point"};

    // A press invokes the control's action directly, which would sail straight
    // past a window-modal sheet that a real click cannot cross. Refuse instead:
    // the caller must deal with the sheet.
    if(!insideSheet) {
      id owner=chosen;
      for(int up=0; up<12 && owner; up++) {
        AXUIElementRef el=(__bridge AXUIElementRef)owner;
        id role=attr(el,@"AXRole");
        if([role isEqual:@"AXSheet"]) { insideSheet=YES; break; }
        if([role isEqual:@"AXWindow"]) {
          for(id kid in attr(el,@"AXChildren")) {
            if([attr((__bridge AXUIElementRef)kid,@"AXRole") isEqual:@"AXSheet"])
              return @{@"found":@NO,@"reason":@"window_blocked_by_modal_sheet"};
          }
          break;
        }
        owner=attr(el,@"AXParent");
      }
    }

    NSDictionary *element=info((__bridge AXUIElementRef)chosen,0,0,@[]);
    if(![args[@"perform"] boolValue]) return @{@"found":@YES,@"element":element,@"action_sent":@NO};
    cuCheckCancelled();
    NSMutableDictionary *receipt=[(scrolling?cuScroll((__bridge AXUIElementRef)chosen,args):cuClick((__bridge AXUIElementRef)chosen,[operation isEqual:@"context"])) mutableCopy];
    receipt[@"found"]=@YES; receipt[@"element"]=element; return receipt;
  }
  /**
   * One pointer gesture, posted to the window server.
   *
   * The tested AppKit fixture dropped process-directed mouse/scroll events.
   * This qualified raw path therefore uses the shared event tap, requiring
   * explicit foreground control. It moves the real cursor, so the gesture
   * runs in one call and restores its starting position when requested.
   * Restoration does not make concurrent desktop use safe.
   */
  if([tool isEqual:@"pointer_sequence"]) {
    CGEventRef probe=CGEventCreate(NULL); CGPoint home=CGEventGetLocation(probe); CFRelease(probe);
    // Shared input is allowed only while the explicitly selected app remains
    // foreground. A new gesture never reactivates it after the user switches.
    NSRunningApplication *front=NSWorkspace.sharedWorkspace.frontmostApplication;
    NSString *before=front.localizedName?:@"";
    BOOL takes=front.processIdentifier!=inputApp.processIdentifier;
    cuCheckCancelled();
    // Activation is a separate, explicit operation. A stale foreground mode
    // must never reclaim focus after the user has switched applications.
    cuRequireForeground(inputApp);
    // AppKit only assembles a drag out of events that look like they came from
    // the input hardware; a NULL-source stream delivers down and up but drops
    // every mouseDragged in between.
    CGEventSourceRef source=CGEventSourceCreate(kCGEventSourceStateHIDSystemState);
    BOOL held[3]={NO,NO,NO};
    CGPoint last=home;
    for(NSDictionary *step in args[@"steps"]) {
      @try { cuCheckCancelled(); cuRequireForeground(inputApp); } @catch(NSException *e) { cuCancelled=1; break; }
      CGEventRef event;
      if(step[@"scroll"]) {
        NSArray *d=step[@"scroll"];
        event=CGEventCreateScrollWheelEvent(source,kCGScrollEventUnitLine,2,[d[1] intValue],[d[0] intValue]);
      } else {
        CGPoint p=CGPointMake([step[@"x"] doubleValue],[step[@"y"] doubleValue]);
        last=p;
        int button=[step[@"button"] intValue], kind=[step[@"type"] intValue];
        if(button>=0 && button<3) {
          if(kind==kCGEventLeftMouseDown || kind==kCGEventRightMouseDown || kind==kCGEventOtherMouseDown) held[button]=YES;
          if(kind==kCGEventLeftMouseUp || kind==kCGEventRightMouseUp || kind==kCGEventOtherMouseUp) held[button]=NO;
        }
        event=CGEventCreateMouseEvent(source,[step[@"type"] unsignedIntValue],p,[step[@"button"] unsignedIntValue]);
        CGEventSetIntegerValueField(event,kCGMouseEventClickState,[step[@"clickState"] longLongValue]);
      }
      CGEventPost(kCGHIDEventTap,event);
      CFRelease(event);
      usleep((useconds_t)([step[@"delayMs"] intValue]?:40)*1000);
    }
    if([args[@"input_lease"] boolValue] && !cuCancelled) {
      for(int button=0;button<3;button++) cuLeaseButtons[button]=held[button];
      cuLeasePoint=last;
      cuLeaseApp=inputApp;
    }
    if(cuCancelled || ![args[@"input_lease"] boolValue]) for(int button=0;button<3;button++) if(held[button]) {
      CGEventType up=button==0?kCGEventLeftMouseUp:button==1?kCGEventRightMouseUp:kCGEventOtherMouseUp;
      CGEventRef event=CGEventCreateMouseEvent(source,up,last,button);
      CGEventPost(kCGHIDEventTap,event); CFRelease(event);
    }
    BOOL restore=[args[@"restore"] boolValue] && !cuCancelled;
    if(restore) {
      usleep(60000);
      CGEventRef back=CGEventCreateMouseEvent(source,kCGEventMouseMoved,home,kCGMouseButtonLeft);
      CGEventPost(kCGHIDEventTap,back); CFRelease(back);
    }
    if(source) CFRelease(source);
    if(cuCancelled) @throw [NSException exceptionWithName:@"cancelled" reason:@"computer request cancelled" userInfo:nil];
    usleep(150000);   // let the window server settle before reading it back
    NSString *after=NSWorkspace.sharedWorkspace.frontmostApplication.localizedName?:@"";
    return @{@"action_sent":@YES,@"pointer_moved":@YES,@"restored":@(restore),
             @"foreground_taken":@(takes),
             @"foreground_before":before,@"foreground_after":after,
             @"home":@{@"x":@(home.x),@"y":@(home.y)}};
  }
  if([tool isEqual:@"scroll"]) {
    cuCheckCancelled();
    CGEventRef event=CGEventCreateScrollWheelEvent(NULL,kCGScrollEventUnitLine,2,[args[@"dy"] intValue],[args[@"dx"] intValue]); CGEventPostToPid(inputApp.processIdentifier,event); CFRelease(event); return @{@"action_sent":@YES};
  }
  if([tool isEqual:@"cursor_position"]) {
    CGEventRef event=CGEventCreate(NULL); CGPoint p=CGEventGetLocation(event); CFRelease(event); return @{@"x":@(p.x),@"y":@(p.y)};
  }
  NSRunningApplication *a=resolve(args[@"app_ref"]?:args[@"target"][@"app_ref"]);
  if(!a) @throw [NSException exceptionWithName:@"app" reason:@"application not found" userInfo:nil];
  AXUIElementRef app=AXUIElementCreateApplication(a.processIdentifier);
  AXUIElementSetMessagingTimeout(app,2.0);
  @try {
    NSArray *ws=attr(app,@"AXWindows")?:@[];
    NSDictionary *identity=@{@"found":@YES,@"name":a.localizedName?:@"",@"pid":@(a.processIdentifier),@"bundle_id":a.bundleIdentifier?:@"",@"frontmost":@(a.active)};
    if([tool isEqual:@"get_app_state"] || [tool isEqual:@"list_windows"]) {
      BOOL truncated=NO;
      NSArray *out=observeElements(app,ws,args,[tool isEqual:@"list_windows"],&truncated);
      NSMutableDictionary *d=[identity mutableCopy]; d[[tool isEqual:@"list_windows"]?@"windows":@"elements"]=out; d[@"truncated"]=@(truncated); return d;
    }
    if([tool isEqual:@"resolve_element"]) {
      NSInteger wi=[args[@"windowIndex"] integerValue];
      id el=wi==-1?attr(app,@"AXMenuBar"):wi==-2?(__bridge id)app:(wi>=0 && wi<ws.count?ws[wi]:nil);
      if(!el) return @{@"found":@NO,@"element":[NSNull null],@"reason":@"window_not_found"};
      for(NSNumber *i in args[@"path"]?:@[]) { NSArray *kids=attr((__bridge AXUIElementRef)el,@"AXChildren"); if(i.unsignedIntegerValue>=kids.count) return @{@"found":@NO,@"element":[NSNull null],@"reason":@"path_not_found"}; el=kids[i.unsignedIntegerValue]; }
      return @{@"found":@YES,@"element":info((__bridge AXUIElementRef)el,0,wi,args[@"path"]?:@[]),@"reason":[NSNull null]};
    }
    NSDictionary *t=args[@"target"]; NSInteger wi=[t[@"windowIndex"] integerValue];
    id el=wi==-1?attr(app,@"AXMenuBar"):wi==-2?(__bridge id)app:(wi>=0 && wi<ws.count?ws[wi]:nil);
    if(!el) @throw [NSException exceptionWithName:@"stale" reason:@"window is no longer available; observe again" userInfo:nil];
    for(NSNumber *i in t[@"path"]) { NSArray *kids=attr((__bridge AXUIElementRef)el,@"AXChildren"); if(i.unsignedIntegerValue>=kids.count) @throw [NSException exceptionWithName:@"stale" reason:@"element is no longer available; observe again" userInfo:nil]; el=kids[i.unsignedIntegerValue]; }
    if(wi>=0) {
      NSArray *sheets=attr((__bridge AXUIElementRef)ws[wi],@"AXSheets");
      if(sheets.count) {
        BOOL inside=NO; id ancestor=el;
        for(int depth=0;ancestor && depth<64;depth++) {
          for(id sheet in sheets) if(CFEqual((__bridge CFTypeRef)ancestor,(__bridge CFTypeRef)sheet)) inside=YES;
          if(inside) break;
          ancestor=attr((__bridge AXUIElementRef)ancestor,@"AXParent");
        }
        if(!inside) @throw [NSException exceptionWithName:@"modal" reason:@"window blocked by modal sheet; observe and handle the dialog first" userInfo:nil];
      }
    }
    cuCheckCancelled();
    if([t[@"type"] isEqual:@"element"]) cuValidateElementIdentity((__bridge AXUIElementRef)el,t);
    if([tool isEqual:@"click_element"]) return cuClick((__bridge AXUIElementRef)el,[args[@"context"] boolValue]);
    if([tool isEqual:@"scroll_element"]) return cuScroll((__bridge AXUIElementRef)el,args);
    AXError e=kAXErrorFailure;
    if([tool isEqual:@"set_value"]) e=AXUIElementSetAttributeValue((__bridge AXUIElementRef)el,kAXValueAttribute,(__bridge CFTypeRef)args[@"value"]);
    else if([tool isEqual:@"select_text"]){ NSArray *r=args[@"text_range"]?:@[@0,@0]; if(r.count!=2 || [r[0] longValue]<0 || [r[1] longValue]<0) @throw [NSException exceptionWithName:@"range" reason:@"text_range must be [start, length], both nonnegative" userInfo:nil]; CFRange range=CFRangeMake([r[0] longValue],[r[1] longValue]); AXValueRef v=AXValueCreate(kAXValueCFRangeType,&range); e=AXUIElementSetAttributeValue((__bridge AXUIElementRef)el,kAXSelectedTextRangeAttribute,v); CFRelease(v); }
    else if([tool isEqual:@"perform_action"]){ CFArrayRef actions=NULL; AXUIElementCopyActionNames((__bridge AXUIElementRef)el,&actions); NSArray *names=CFBridgingRelease(actions); if(![names containsObject:args[@"action"]]) @throw [NSException exceptionWithName:@"action" reason:@"action is not advertised by this element" userInfo:nil]; cuCheckCancelled(); e=AXUIElementPerformAction((__bridge AXUIElementRef)el,(__bridge CFStringRef)args[@"action"]); }
    if(e!=kAXErrorSuccess) @throw [NSException exceptionWithName:@"action" reason:[NSString stringWithFormat:@"accessibility action failed: %d",e] userInfo:nil];
    return @{@"action_sent":@YES,@"strategy":@"a11y"};
  } @finally { CFRelease(app); }
}
int main(int argc, const char **argv){ @autoreleasepool {
  signal(SIGTERM,cuCancel); signal(SIGINT,cuCancel); signal(SIGPIPE,SIG_IGN);
  @try { if(argc!=2) @throw [NSException exceptionWithName:@"args" reason:@"expected one JSON argument" userInfo:nil];
    NSError *error=nil; id p=[NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:argv[1]] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:&error];
    if(![p isKindOfClass:NSDictionary.class]) @throw [NSException exceptionWithName:@"json" reason:@"invalid request" userInfo:nil];
    id result=execute(p);
    if([p[@"args"][@"input_lease"] boolValue]) { NSMutableDictionary *ack=[result mutableCopy]; ack[@"input_lease"]=@YES; result=ack; }
    NSData *data=[NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingFragmentsAllowed error:&error];
    if(!data) @throw [NSException exceptionWithName:@"json" reason:error.localizedDescription userInfo:nil];
    puts([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String); fflush(stdout);
    if([p[@"args"][@"input_lease"] boolValue]) cuWaitForLease();
    return 0;
  } @catch(NSException *e){ cuReleaseLease(); fprintf(stderr,"%s\n",e.reason.UTF8String); return 1; }
} }
