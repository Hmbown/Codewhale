#import <Cocoa/Cocoa.h>
#import <ApplicationServices/ApplicationServices.h>
#include <unistd.h>
#import "darwin-recording.h"

static id attr(AXUIElementRef el, NSString *name) {
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
  if(AXUIElementCopyActionNames(el,&actions)==kAXErrorSuccess) d[@"actions"]=CFBridgingRelease(actions);
  else d[@"actions"]=@[];
  return d;
}
static void walk(AXUIElementRef el, NSInteger win, NSArray *path, NSInteger depth, NSInteger limit, NSInteger max, NSMutableArray *out, BOOL *truncated) {
  if(out.count>=max || depth>limit){ *truncated=YES; return; }
  [out addObject:info(el,out.count,win,path)];
  NSArray *kids=attr(el,@"AXChildren");
  for(NSUInteger i=0;i<kids.count;i++) {
    if(out.count>=max){ *truncated=YES; break; }
    walk((__bridge AXUIElementRef)kids[i],win,[path arrayByAddingObject:@(i)],depth+1,limit,max,out,truncated);
  }
}
static BOOL cuFrame(AXUIElementRef el, CGRect *out) {
  NSDictionary *p=geometry(attr(el,@"AXPosition"),NO), *z=geometry(attr(el,@"AXSize"),YES);
  if(!p || !z) return NO;
  *out=CGRectMake([p[@"x"] doubleValue],[p[@"y"] doubleValue],[z[@"w"] doubleValue],[z[@"h"] doubleValue]);
  return YES;
}
static BOOL cuPressable(AXUIElementRef el) {
  CFArrayRef names=NULL;
  NSArray *actions=AXUIElementCopyActionNames(el,&names)==kAXErrorSuccess?CFBridgingRelease(names):@[];
  if(![actions containsObject:@"AXPress"]) return NO;
  id enabled=attr(el,@"AXEnabled");
  return ![enabled isKindOfClass:NSNumber.class] || [enabled boolValue];
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
static void cuSearch(AXUIElementRef el, CGPoint p, int depth, int *budget, id *best, double *bestArea) {
  if(depth>24 || (*budget)--<=0) return;
  CGRect frame;
  if(cuFrame(el,&frame)) {
    // Children are laid out inside their parent (and clipped when they are
    // not), so a frame that misses the point prunes the whole subtree.
    if(!CGRectContainsPoint(frame,p)) return;
    double area=frame.size.width*frame.size.height;
    if(cuPressable(el) && (!*best || area<=*bestArea)) { *best=(__bridge id)el; *bestArea=area; }
  }
  for(id kid in attr(el,@"AXChildren")) cuSearch((__bridge AXUIElementRef)kid,p,depth+1,budget,best,bestArea);
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
  if(![ref isKindOfClass:NSDictionary.class]) ref=@{};
  if(!ref.count) return NSWorkspace.sharedWorkspace.frontmostApplication;
  NSString *bundle=[ref[@"bundle_id"] length]?ref[@"bundle_id"]:nil, *name=[ref[@"name"] length]?ref[@"name"]:nil;
  if(!ref[@"pid"] && !bundle && !name) return NSWorkspace.sharedWorkspace.frontmostApplication;
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
  CGEventKeyboardSetUnicodeString(event,text.length,chars); free(chars); return event;
}
static id execute(NSDictionary *p) {
  NSString *tool=p[@"tool"]; NSDictionary *args=p[@"args"]?:@{};
  if([tool isEqual:@"record"]) return cuRecord(args);
#ifdef CU_TEST
  if([tool isEqual:@"inspect_text_event"]) {
    CGEventRef event=textEvent(args[@"text"],YES); UniChar chars[4096]; UniCharCount length=0;
    CGEventKeyboardGetUnicodeString(event,4096,&length,chars); CFRelease(event);
    return @{@"text":[NSString stringWithCharacters:chars length:length]};
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
    NSRunningApplication *a=resolve(args[@"app_ref"]?:args[@"input_app_ref"]?:@{});
    if(!a) @throw [NSException exceptionWithName:@"app" reason:@"application not found" userInfo:nil];
    AXUIElementRef ax=AXUIElementCreateApplication(a.processIdentifier);
    NSArray *axWindows=attr(ax,@"AXWindows");
    NSDictionary *preferred=axWindows.count?geometry(attr((__bridge AXUIElementRef)axWindows[0],@"AXPosition"),NO):nil;
    CFRelease(ax);
    NSArray *windows=CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionAll,kCGNullWindowID));
    for(NSDictionary *w in windows) {
      if([w[(__bridge NSString *)kCGWindowOwnerPID] intValue]!=a.processIdentifier || [w[(__bridge NSString *)kCGWindowLayer] intValue]!=0) continue;
      CGRect b; if(!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)w[(__bridge NSString *)kCGWindowBounds],&b) || b.size.width<1 || b.size.height<1) continue;
      if(preferred && (fabs(b.origin.x-[preferred[@"x"] doubleValue])>1 || fabs(b.origin.y-[preferred[@"y"] doubleValue])>1)) continue;
      return @{@"window_id":w[(__bridge NSString *)kCGWindowNumber],@"name":a.localizedName?:@"App",@"points":@{@"x":@(b.origin.x),@"y":@(b.origin.y),@"w":@(b.size.width),@"h":@(b.size.height)}};
    }
    @throw [NSException exceptionWithName:@"window" reason:@"application has no capturable window" userInfo:nil];
  }
  // Which application owns the point a pointer event would land on. A global
  // pointer event goes to whatever is on top, so this is what stops a click
  // meant for the agent's app from landing in the user's window.
  if([tool isEqual:@"window_at_point"]) {
    CGPoint p=CGPointMake([args[@"x"] doubleValue],[args[@"y"] doubleValue]);
    NSArray *windows=CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly|kCGWindowListExcludeDesktopElements,kCGNullWindowID));
    // The bound application, when one is known: its own panels and menus sit
    // above the normal window layer and are legitimate targets.
    pid_t bound=[args[@"input_app_ref"][@"pid"] intValue];
    NSMutableArray *skipped=[NSMutableArray array];
    for(NSDictionary *w in windows) {          // front to back
      CGRect b;
      if(!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)w[(__bridge NSString *)kCGWindowBounds],&b)) continue;
      if(!CGRectContainsPoint(b,p)) continue;
      pid_t owner=[w[(__bridge NSString *)kCGWindowOwnerPID] intValue];
      NSString *name=w[(__bridge NSString *)kCGWindowOwnerName]?:@"";
      NSNumber *alpha=w[(__bridge NSString *)kCGWindowAlpha], *layer=w[(__bridge NSString *)kCGWindowLayer]?:@0;
      // Invisible overlays and the system furniture that floats over every
      // point (Dock, menu bar, notification layer) are not what a coordinate
      // means; a normal-layer window from any application is.
      if(alpha && [alpha doubleValue]<=0.01) { [skipped addObject:@{@"owner":name,@"why":@"transparent"}]; continue; }
      if([layer intValue]!=0 && owner!=bound) { [skipped addObject:@{@"owner":name,@"why":[NSString stringWithFormat:@"layer %d",[layer intValue]]}]; continue; }
      return @{@"found":@YES,@"owner_pid":@(owner),@"owner_name":name,
               @"window_id":w[(__bridge NSString *)kCGWindowNumber]?:@0,@"layer":layer,
               @"skipped":skipped};
    }
    return @{@"found":@NO,@"skipped":skipped};
  }
  if([tool isEqual:@"app_info"]) {
    NSRunningApplication *a=resolve(args[@"app_ref"]?:@{});
    if(!a) @throw [NSException exceptionWithName:@"app" reason:@"application not found" userInfo:nil];
    if([args[@"activate"] boolValue] && !axActivate(a.processIdentifier)) [a activateWithOptions:0];
    return @{@"found":@YES,@"name":a.localizedName?:@"",@"pid":@(a.processIdentifier),@"bundle_id":a.bundleIdentifier?:@"",@"frontmost":@(a.active)};
  }
  NSRunningApplication *inputApp=nil;
  if([@[@"type",@"key_event",@"mouse_event",@"scroll",@"hit_test",@"pointer_sequence"] containsObject:tool]) {
    if(![args[@"input_app_ref"] isKindOfClass:NSDictionary.class]) @throw [NSException exceptionWithName:@"focus" reason:@"open_application first to bind the input destination" userInfo:nil];
    inputApp=resolve(args[@"input_app_ref"]);
    if(!inputApp || inputApp.terminated) @throw [NSException exceptionWithName:@"focus" reason:@"input application is no longer running; open_application again" userInfo:nil];
  }
  if(!AXIsProcessTrusted()) @throw [NSException exceptionWithName:@"permission" reason:@"Accessibility permission is missing for Codewhale Computer Use (or the direct host)." userInfo:nil];
  if([tool isEqual:@"type"]) {
    NSString *text=args[@"text"];
    if(![text isKindOfClass:NSString.class]) @throw [NSException exceptionWithName:@"text" reason:@"text must be a string" userInfo:nil];
    // One grapheme per event, the way a keyboard delivers them. Batching
    // several into one CGEventKeyboardSetUnicodeString is faster but Electron
    // apps coalesce the pending payload and keep only the final batch, so a
    // typed string silently arrives truncated to its tail.
    for(NSUInteger i=0;i<text.length;) {
      NSRange range=[text rangeOfComposedCharacterSequencesForRange:NSMakeRange(i,1)];
      NSString *chunk=[text substringWithRange:range];
      for(int down=1;down>=0;down--){ CGEventRef event=textEvent(chunk,down); CGEventPostToPid(inputApp.processIdentifier,event); CFRelease(event); }
      i=NSMaxRange(range); usleep(10000);
    }
    return @{@"action_sent":@YES,@"chars":@(text.length),@"strategy":@"unicode-events"};
  }
  if([tool isEqual:@"key_event"]) {
    CGEventRef event=CGEventCreateKeyboardEvent(NULL,[args[@"code"] unsignedShortValue],[args[@"down"] boolValue]);
    CGEventSetFlags(event,[args[@"flags"] unsignedLongLongValue]); CGEventPostToPid(inputApp.processIdentifier,event); CFRelease(event); return @{@"action_sent":@YES};
  }
  if([tool isEqual:@"mouse_event"]) {
    CGPoint p=CGPointMake([args[@"x"] doubleValue],[args[@"y"] doubleValue]);
    CGEventRef event=CGEventCreateMouseEvent(NULL,[args[@"type"] unsignedIntValue],p,[args[@"button"] unsignedIntValue]);
    CGEventSetIntegerValueField(event,kCGMouseEventClickState,[args[@"clickState"] longLongValue]);
    // A pointer event posted to a process carries no window, and AppKit drops
    // what it cannot route. Naming the window under the point is what lets a
    // background application receive it without the pointer ever moving.
    if([args[@"windowNumber"] longLongValue]>0) {
      CGEventSetIntegerValueField(event,91,[args[@"windowNumber"] longLongValue]);
      CGEventSetIntegerValueField(event,92,[args[@"windowNumber"] longLongValue]);
    }
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
      else { CFRelease(raw); CFRelease(appEl); return @{@"found":@NO,@"reason":@"point_owned_by_another_process"}; }
    }

    id chosen=nil;
    BOOL insideSheet=NO;
    // 1. The element under the point, or the nearest ancestor that can be
    //    pressed — a label inside a button is the common case.
    for(id cur=hit; cur && !chosen;) {
      AXUIElementRef el=(__bridge AXUIElementRef)cur;
      id role=attr(el,@"AXRole");
      if([role isEqual:@"AXSheet"]) insideSheet=YES;
      if([role isEqual:@"AXWindow"] || [role isEqual:@"AXApplication"]) break;
      if(cuPressable(el)) { chosen=cur; break; }
      cur=attr(el,@"AXParent");
    }
    // 2. Otherwise search downward for the smallest control covering the point.
    if(!chosen) {
      int budget=1500; double area=0; id best=nil;
      if(hit) cuSearch((__bridge AXUIElementRef)hit,p,0,&budget,&best,&area);
      else for(id w in attr(appEl,@"AXWindows")) cuSearch((__bridge AXUIElementRef)w,p,0,&budget,&best,&area);
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
    if(![args[@"perform"] boolValue]) return @{@"found":@YES,@"element":element,@"action":@"AXPress",@"action_sent":@NO};
    AXError pe=AXUIElementPerformAction((__bridge AXUIElementRef)chosen,CFSTR("AXPress"));
    if(pe!=kAXErrorSuccess) return @{@"found":@YES,@"element":element,@"action":@"AXPress",@"action_sent":@NO,@"reason":[NSString stringWithFormat:@"press_failed_%d",pe]};
    return @{@"found":@YES,@"element":element,@"action":@"AXPress",@"action_sent":@YES};
  }
  /**
   * One pointer gesture, posted to the window server.
   *
   * macOS delivers keyboard events to a process but silently drops pointer and
   * scroll events posted the same way (measured on CGEventPostToPid with both
   * event sources and on CGEventPostToPSN), so a pointer gesture that has no
   * accessibility equivalent has to travel through the shared event tap. That
   * moves the real cursor, so the whole gesture runs in one call and the
   * pointer is put back where the user left it.
   */
  if([tool isEqual:@"pointer_sequence"]) {
    CGEventRef probe=CGEventCreate(NULL); CGPoint home=CGEventGetLocation(probe); CFRelease(probe);
    // A global pointer event landing in an inactive application activates it,
    // and AppKit swallows that first mouse-down instead of delivering it — a
    // drag would silently lose its press. So the foreground is taken up front,
    // deliberately. It cannot be handed back: macOS 14+ ignores activation
    // requests from a process that is not itself frontmost (measured for both
    // -[NSRunningApplication activateWithOptions:] and AXFrontmost), and the
    // request only lands here because the click is about to arrive anyway.
    // The cost is reported in the receipt, never hidden.
    NSRunningApplication *front=NSWorkspace.sharedWorkspace.frontmostApplication;
    NSString *before=front.localizedName?:@"";
    // A gesture into an application that is not already frontmost brings it
    // forward — by our own request when that is permitted, and by the click
    // itself when it is not. Either way the foreground is taken, so say so
    // from the fact that decides it rather than from a frontmost read that
    // the window server may not have caught up with yet.
    BOOL takes=front.processIdentifier!=inputApp.processIdentifier;
    if(takes) {
      axActivate(inputApp.processIdentifier);
      for(int i=0;i<20;i++) {
        if(NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier==inputApp.processIdentifier) break;
        usleep(25000);
      }
    }
    // AppKit only assembles a drag out of events that look like they came from
    // the input hardware; a NULL-source stream delivers down and up but drops
    // every mouseDragged in between.
    CGEventSourceRef source=CGEventSourceCreate(kCGEventSourceStateHIDSystemState);
    for(NSDictionary *step in args[@"steps"]) {
      CGEventRef event;
      if(step[@"scroll"]) {
        NSArray *d=step[@"scroll"];
        event=CGEventCreateScrollWheelEvent(source,kCGScrollEventUnitLine,2,[d[1] intValue],[d[0] intValue]);
      } else {
        CGPoint p=CGPointMake([step[@"x"] doubleValue],[step[@"y"] doubleValue]);
        event=CGEventCreateMouseEvent(source,[step[@"type"] unsignedIntValue],p,[step[@"button"] unsignedIntValue]);
        CGEventSetIntegerValueField(event,kCGMouseEventClickState,[step[@"clickState"] longLongValue]);
      }
      CGEventPost(kCGHIDEventTap,event);
      CFRelease(event);
      usleep((useconds_t)([step[@"delayMs"] intValue]?:40)*1000);
    }
    BOOL restore=[args[@"restore"] boolValue];
    if(restore) {
      usleep(60000);
      CGEventRef back=CGEventCreateMouseEvent(source,kCGEventMouseMoved,home,kCGMouseButtonLeft);
      CGEventPost(kCGHIDEventTap,back); CFRelease(back);
    }
    if(source) CFRelease(source);
    usleep(150000);   // let the window server settle before reading it back
    NSString *after=NSWorkspace.sharedWorkspace.frontmostApplication.localizedName?:@"";
    return @{@"action_sent":@YES,@"pointer_moved":@YES,@"restored":@(restore),
             @"foreground_taken":@(takes),
             @"foreground_before":before,@"foreground_after":after,
             @"home":@{@"x":@(home.x),@"y":@(home.y)}};
  }
  if([tool isEqual:@"scroll"]) {
    CGEventRef event=CGEventCreateScrollWheelEvent(NULL,kCGScrollEventUnitLine,2,[args[@"dy"] intValue],[args[@"dx"] intValue]); CGEventPostToPid(inputApp.processIdentifier,event); CFRelease(event); return @{@"action_sent":@YES};
  }
  if([tool isEqual:@"cursor_position"]) {
    CGEventRef event=CGEventCreate(NULL); CGPoint p=CGEventGetLocation(event); CFRelease(event); return @{@"x":@(p.x),@"y":@(p.y)};
  }
  NSRunningApplication *a=resolve(args[@"app_ref"]?:args[@"target"][@"app_ref"]?:@{});
  if(!a) @throw [NSException exceptionWithName:@"app" reason:@"application not found" userInfo:nil];
  AXUIElementRef app=AXUIElementCreateApplication(a.processIdentifier);
  AXUIElementSetMessagingTimeout(app,2.0);
  @try {
    NSArray *ws=attr(app,@"AXWindows")?:@[];
    NSDictionary *identity=@{@"found":@YES,@"name":a.localizedName?:@"",@"pid":@(a.processIdentifier),@"bundle_id":a.bundleIdentifier?:@"",@"frontmost":@(a.active)};
    if([tool isEqual:@"get_app_state"] || [tool isEqual:@"list_windows"]) {
      NSMutableArray *out=[NSMutableArray array]; BOOL truncated=NO;
      for(NSUInteger i=0;i<ws.count;i++) {
        if(args[@"window_id"] && i!=[args[@"window_id"] unsignedIntegerValue]) continue;
        if([tool isEqual:@"list_windows"]){ NSMutableDictionary *d=[info((__bridge AXUIElementRef)ws[i],i,i,@[]) mutableCopy]; d[@"title"]=d[@"label"]?:@""; [out addObject:d]; }
        else walk((__bridge AXUIElementRef)ws[i],i,@[],0,[args[@"detail"] isEqual:@"full"]?16:10,[args[@"detail"] isEqual:@"full"]?800:400,out,&truncated);
      }
      NSMutableDictionary *d=[identity mutableCopy]; d[[tool isEqual:@"list_windows"]?@"windows":@"elements"]=out; d[@"truncated"]=@(truncated); return d;
    }
    if([tool isEqual:@"resolve_element"]) {
      NSUInteger wi=[args[@"windowIndex"] unsignedIntegerValue];
      if(wi>=ws.count) return @{@"found":@NO,@"element":[NSNull null],@"reason":@"window_not_found"};
      id el=ws[wi];
      for(NSNumber *i in args[@"path"]?:@[]) { NSArray *kids=attr((__bridge AXUIElementRef)el,@"AXChildren"); if(i.unsignedIntegerValue>=kids.count) return @{@"found":@NO,@"element":[NSNull null],@"reason":@"path_not_found"}; el=kids[i.unsignedIntegerValue]; }
      return @{@"found":@YES,@"element":info((__bridge AXUIElementRef)el,0,wi,args[@"path"]?:@[]),@"reason":[NSNull null]};
    }
    NSDictionary *t=args[@"target"]; NSUInteger wi=[t[@"windowIndex"] unsignedIntegerValue];
    if(wi>=ws.count) @throw [NSException exceptionWithName:@"stale" reason:@"window is no longer available; observe again" userInfo:nil];
    id el=ws[wi];
    for(NSNumber *i in t[@"path"]) { NSArray *kids=attr((__bridge AXUIElementRef)el,@"AXChildren"); if(i.unsignedIntegerValue>=kids.count) @throw [NSException exceptionWithName:@"stale" reason:@"element is no longer available; observe again" userInfo:nil]; el=kids[i.unsignedIntegerValue]; }
    AXError e=kAXErrorFailure;
    if([tool isEqual:@"set_value"]) e=AXUIElementSetAttributeValue((__bridge AXUIElementRef)el,kAXValueAttribute,(__bridge CFTypeRef)args[@"value"]);
    else if([tool isEqual:@"select_text"]){ NSArray *r=args[@"text_range"]?:@[@0,@0]; if(r.count!=2 || [r[0] longValue]<0 || [r[1] longValue]<0) @throw [NSException exceptionWithName:@"range" reason:@"text_range must be [start, length], both nonnegative" userInfo:nil]; CFRange range=CFRangeMake([r[0] longValue],[r[1] longValue]); AXValueRef v=AXValueCreate(kAXValueCFRangeType,&range); e=AXUIElementSetAttributeValue((__bridge AXUIElementRef)el,kAXSelectedTextRangeAttribute,v); CFRelease(v); }
    else if([tool isEqual:@"perform_action"]){ CFArrayRef actions=NULL; AXUIElementCopyActionNames((__bridge AXUIElementRef)el,&actions); NSArray *names=CFBridgingRelease(actions); if(![names containsObject:args[@"action"]]) @throw [NSException exceptionWithName:@"action" reason:@"action is not advertised by this element" userInfo:nil]; e=AXUIElementPerformAction((__bridge AXUIElementRef)el,(__bridge CFStringRef)args[@"action"]); }
    if(e!=kAXErrorSuccess) @throw [NSException exceptionWithName:@"action" reason:[NSString stringWithFormat:@"accessibility action failed: %d",e] userInfo:nil];
    return @{@"action_sent":@YES,@"strategy":@"a11y"};
  } @finally { CFRelease(app); }
}
int main(int argc, const char **argv){ @autoreleasepool {
  @try { if(argc!=2) @throw [NSException exceptionWithName:@"args" reason:@"expected one JSON argument" userInfo:nil];
    NSError *error=nil; id p=[NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:argv[1]] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:&error];
    if(![p isKindOfClass:NSDictionary.class]) @throw [NSException exceptionWithName:@"json" reason:@"invalid request" userInfo:nil];
    id result=execute(p); NSData *data=[NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingFragmentsAllowed error:&error];
    if(!data) @throw [NSException exceptionWithName:@"json" reason:error.localizedDescription userInfo:nil];
    puts([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String); return 0;
  } @catch(NSException *e){ fprintf(stderr,"%s\n",e.reason.UTF8String); return 1; }
} }
