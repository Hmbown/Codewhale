#import <Cocoa/Cocoa.h>
#import <ApplicationServices/ApplicationServices.h>
#import <dlfcn.h>
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

/**
 * Window-routed background pointer.
 *
 * Process-directed mouse events (CGEventPostToPid) never reach AppKit views,
 * and posting to the HID tap moves the user's real cursor. The route that
 * delivers is the WindowServer's event-record channel: a CGEvent carrying the
 * target window's id (fields 0x33/0x5b/0x5c) plus a window-space location is
 * posted as its raw event record via SLPSPostEventRecordTo. AppKit only
 * dispatches mouse events to views whose window holds key status, so a
 * hand-built window-focus record is posted first — the target app never
 * becomes frontmost, the menu bar never flickers, Chromium's accessibility
 * tree is not torn down, and the operator's keystrokes stay theirs. The real
 * cursor never moves, so receipts can say pointer_moved:false,
 * front_lease:false.
 */
typedef OSStatus (*cuGetFrontFn)(ProcessSerialNumber *);
typedef OSStatus (*cuGetPSNFn)(pid_t, ProcessSerialNumber *);
typedef OSStatus (*cuSetFrontFn)(ProcessSerialNumber *, uint32_t, uint32_t);
typedef OSStatus (*cuPostRecordFn)(ProcessSerialNumber *, const void *);
typedef void (*cuSetWinLocFn)(CGEventRef, CGPoint);
static BOOL axActivate(pid_t pid);
static BOOL cuBgResolved = NO;
static cuGetFrontFn cuGetFront;
static cuGetPSNFn cuGetPSN;
static cuSetFrontFn cuSetFront;
static cuPostRecordFn cuPostRecord;
static cuSetWinLocFn cuSetWinLoc;
static BOOL cuResolveBgPointer(void) {
  if(cuBgResolved) return cuGetFront && cuGetPSN && cuSetFront && cuPostRecord && cuSetWinLoc;
  cuBgResolved = YES;
  void *sl = dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_LAZY);
  void *hs = dlopen("/System/Library/Frameworks/ApplicationServices.framework/Frameworks/HIServices.framework/HIServices", RTLD_LAZY);
  if(sl) {
    cuGetFront = (cuGetFrontFn)dlsym(sl, "_SLPSGetFrontProcess");
    cuSetFront = (cuSetFrontFn)dlsym(sl, "SLPSSetFrontProcessWithOptions");
    cuPostRecord = (cuPostRecordFn)dlsym(sl, "SLPSPostEventRecordTo");
    cuSetWinLoc = (cuSetWinLocFn)dlsym(sl, "CGEventSetWindowLocation");
  }
  if(hs) cuGetPSN = (cuGetPSNFn)dlsym(hs, "GetProcessForPID");
  return cuGetFront && cuGetPSN && cuSetFront && cuPostRecord && cuSetWinLoc;
}
/** Smallest layer-0 window of pid containing p (a sheet beats its parent). */
static BOOL cuWindowAtPointForPid(pid_t pid, CGPoint p, uint32_t *outWin, CGRect *outFrame) {
  NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionAll, kCGNullWindowID));
  double bestArea = 0;
  BOOL found = NO;
  for(NSDictionary *w in windows) {
    if([w[(__bridge NSString *)kCGWindowOwnerPID] intValue] != pid) continue;
    if([w[(__bridge NSString *)kCGWindowLayer] intValue] != 0) continue;
    NSNumber *alpha = w[(__bridge NSString *)kCGWindowAlpha];
    if(alpha && [alpha doubleValue] <= 0) continue;
    CGRect b;
    if(!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)w[(__bridge NSString *)kCGWindowBounds], &b)) continue;
    if(b.size.width < 1 || b.size.height < 1 || !CGRectContainsPoint(b, p)) continue;
    double area = b.size.width * b.size.height;
    if(!found || area < bestArea) {
      found = YES; bestArea = area;
      *outWin = [w[(__bridge NSString *)kCGWindowNumber] unsignedIntValue];
      *outFrame = b;
    }
  }
  return found;
}
/**
 * State for a window-routed action. Measured on macOS 26.1: the record
 * channel only delivers to views when the target window is key, and the
 * front-process lease (kCPSNoWindows-style options, no windows raised) is
 * what makes it key — the window-focus record alone makes it main, which
 * leaves events arriving at the process and swallowed by first-mouse
 * semantics. The lease is taken for every gesture and restored in @finally;
 * menus opened during it are held across calls (watchdog-capped) because a
 * menu closes the moment the lease ends. Chromium rebuilds its AX tree
 * lazily across the first transitions, so observes poll through the rebuild.
 */
typedef struct { ProcessSerialNumber frontPSN, targetPSN; pid_t frontPid; pid_t targetPid; BOOL swapped; double t0, idleBefore, idleAfter, leaseMs; } cuBgLease;
static BOOL axActivate(pid_t pid);
static BOOL cuFrontmostIsPid(pid_t pid);
static id attr(AXUIElementRef el, NSString *name);
static void axPrepare(AXUIElementRef app);
static BOOL cuBgLeaseBegin(NSRunningApplication *inputApp, uint32_t winNum, BOOL swap, cuBgLease *lease, NSString **why) {
  lease->targetPid = inputApp.processIdentifier;
  lease->swapped = NO;
  lease->t0 = lease->idleBefore = lease->idleAfter = lease->leaseMs = 0;
  if(cuGetPSN(lease->targetPid, &lease->targetPSN) != 0) {
    *why = @"could not resolve the process serial number for a window-routed action; no input was sent";
    return NO;
  }
  cuCheckCancelled();
  if(swap && !cuFrontmostIsPid(lease->targetPid)) {
    // An already-frontmost target needs no swap: the record already addresses
    // the window, and setting an app front of itself changes nothing. Skipping
    // it keeps receipts truthful (front_lease:false) and avoids a restore
    // attempt for focus that was never borrowed.
    NSRunningApplication *frontApp = NSWorkspace.sharedWorkspace.frontmostApplication;
    lease->frontPid = frontApp.processIdentifier;
    if(cuGetFront(&lease->frontPSN) != 0 || cuSetFront(&lease->targetPSN, 0, 0x400) != 0) {
      *why = @"the window server refused the background focus lease; no input was sent";
      return NO;
    }
    lease->swapped = YES;
    // Interference accounting (SHA-6643): the borrow window and the HID idle
    // clock around it. Synthesized events do not tick this clock (verified
    // 2026-09-17: a posted key leaves it advancing), so a clock that fails to
    // advance across the window means hardware input arrived mid-lease.
    lease->t0 = CFAbsoluteTimeGetCurrent();
    lease->idleBefore = CGEventSourceSecondsSinceLastEventType(kCGEventSourceStateHIDSystemState, kCGAnyInputEventType);
  }
  uint8_t rec[0xf8];
  memset(rec, 0, sizeof(rec));
  rec[0x24] = 0xf8; rec[0x28] = 0x0d;
  rec[0x5c] = (winNum >> 24) & 0xff; rec[0x5d] = (winNum >> 16) & 0xff;
  rec[0x5e] = (winNum >> 8) & 0xff;  rec[0x5f] = winNum & 0xff;
  rec[0xaa] = 0x01;
  cuPostRecord(&lease->targetPSN, rec);
  usleep(30000);
  return YES;
}
static void cuLeaseAccounting(NSMutableDictionary *receipt, cuBgLease *lease) {
  // No numbers when nothing was borrowed, or when the lease is still held
  // across calls (menu path): a zero window would claim an instant lease.
  // Millisecond precision: these ride every lease receipt and its trajectory.
  if(!lease->swapped || lease->leaseMs <= 0) return;
  receipt[@"lease_ms"] = @(round(lease->leaseMs * 1000.0) / 1000.0);
  receipt[@"idle_before_s"] = @(round(lease->idleBefore * 1000.0) / 1000.0);
  receipt[@"idle_after_s"] = @(round(lease->idleAfter * 1000.0) / 1000.0);
}
static BOOL cuBgLeaseEnd(cuBgLease *lease) {
  if(!lease->swapped) return YES; // nothing was borrowed
  cuSetFront(&lease->frontPSN, 0, 0x400);
  // NSWorkspace's frontmost view is stale in a one-shot helper; the SLS
  // front-process read is authoritative. Re-assert through AX while the
  // lease is still visible. The outcome is reported, not assumed: a failed
  // restore means the person's next keystrokes land in the wrong app.
  for(int i = 0; i < 20; i++) {
    if(!cuFrontmostIsPid(lease->targetPid)) break;
    axActivate(lease->frontPid);
    usleep(50000);
  }
  // Measured after restore: the borrow window runs take -> handed back, and
  // input that lands during a struggling restore counts as mid-lease.
  lease->idleAfter = CGEventSourceSecondsSinceLastEventType(kCGEventSourceStateHIDSystemState, kCGAnyInputEventType);
  lease->leaseMs = (CFAbsoluteTimeGetCurrent() - lease->t0) * 1000.0;
  return !cuFrontmostIsPid(lease->targetPid);
}
static BOOL cuFrontmostIsPid(pid_t pid) {
  ProcessSerialNumber front, want;
  if(cuGetFront(&front) == 0 && cuGetPSN(pid, &want) == 0)
    return front.highLongOfPSN == want.highLongOfPSN && front.lowLongOfPSN == want.lowLongOfPSN;
  [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];
  return NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == pid;
}
/**
 * A menu opened under a front-process lease closes the moment the lease
 * ends, so for those (Chromium) the lease is held in a state file across
 * calls and given back by the next raw-input call or a 6 s watchdog —
 * and only while the target is still frontmost: the user taking another
 * app in the meantime is a choice, never something to yank back.
 */
static NSString *cuFrontLeaseFile(void) {
  return [NSHomeDirectory() stringByAppendingPathComponent:@".codewhale-cu/front-lease.json"];
}
static void cuFrontLeaseRestoreIfHeld(void) {
  NSString *file = cuFrontLeaseFile();
  NSData *data = [NSData dataWithContentsOfFile:file];
  if(!data) return;
  NSDictionary *held = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
  if(![held isKindOfClass:NSDictionary.class]) return;
  [NSFileManager.defaultManager removeItemAtPath:file error:nil];
  if(!cuResolveBgPointer()) return;
  pid_t targetPid = [held[@"targetPid"] intValue];
  if(!cuFrontmostIsPid(targetPid)) return;
  ProcessSerialNumber psn = { (UInt32)[held[@"frontHi"] unsignedIntValue], (UInt32)[held[@"frontLo"] unsignedIntValue] };
  cuSetFront(&psn, 0, 0x400);
  for(int i = 0; i < 10; i++) {
    if(!cuFrontmostIsPid(targetPid)) break;
    axActivate([held[@"frontPid"] intValue]);
    usleep(50000);
  }
}
static BOOL cuFrontLeaseHeldForPid(pid_t pid) {
  NSData *data = [NSData dataWithContentsOfFile:cuFrontLeaseFile()];
  if(!data) return NO;
  NSDictionary *held = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
  return [held isKindOfClass:NSDictionary.class] && [held[@"targetPid"] intValue] == pid;
}
static BOOL cuMenuOpenForApp(pid_t pid) {
  AXUIElementRef app = AXUIElementCreateApplication(pid);
  AXUIElementSetMessagingTimeout(app, 1.0);
  axPrepare(app);
  // Chromium vends an open select popup inside the window's subtree rather
  // than as an app-level AXMenu, so both shapes are searched, bounded.
  int budget = 400;
  NSMutableArray *stack = [NSMutableArray array];
  [stack addObjectsFromArray:attr(app, @"AXChildren") ?: @[]];
  [stack addObjectsFromArray:attr(app, @"AXWindows") ?: @[]];
  BOOL open = NO;
  while(stack.count && budget-- > 0 && !open) {
    id node = stack.lastObject;
    [stack removeLastObject];
    NSString *role = attr((__bridge AXUIElementRef)node, @"AXRole");
    if([role isEqual:@"AXMenu"]) { open = YES; break; }
    if([role isEqual:@"AXMenuBar"] || [role isEqual:@"AXMenuBarItem"]) continue;
    [stack addObjectsFromArray:attr((__bridge AXUIElementRef)node, @"AXChildren") ?: @[]];
  }
  CFRelease(app);
  return open;
}
static void cuFrontLeaseHold(cuBgLease *lease) {
  NSString *token = NSUUID.UUID.UUIDString;
  // Menu items take seconds to reappear in Chromium's rebuilt tree; the
  // observe+pick must fit inside the hold.
  NSNumber *deadline = @((long long)([NSDate new].timeIntervalSince1970 * 1000) + 15000);
  NSDictionary *state = @{ @"token": token, @"targetPid": @(lease->targetPid), @"frontPid": @(lease->frontPid),
                           @"frontHi": @(lease->frontPSN.highLongOfPSN), @"frontLo": @(lease->frontPSN.lowLongOfPSN),
                           @"deadline": deadline };
  NSString *file = cuFrontLeaseFile();
  [NSFileManager.defaultManager createDirectoryAtPath:file.stringByDeletingLastPathComponent withIntermediateDirectories:YES attributes:@{ NSFilePosixPermissions: @0700 } error:nil];
  NSData *data = [NSJSONSerialization dataWithJSONObject:state options:0 error:nil];
  if(!data || ![data writeToFile:file atomically:YES]) { cuBgLeaseEnd(lease); return; }
  NSDictionary *req = @{ @"tool": @"front_lease_watchdog", @"args": @{ @"token": token, @"deadline": deadline } };
  NSData *reqData = [NSJSONSerialization dataWithJSONObject:req options:0 error:nil];
  if(reqData) {
    NSTask *watch = [NSTask new];
    watch.executableURL = [NSURL fileURLWithPath:NSProcessInfo.processInfo.arguments[0]];
    watch.arguments = @[ [[NSString alloc] initWithData:reqData encoding:NSUTF8StringEncoding] ];
    watch.standardInput = NSFileHandle.fileHandleWithNullDevice;
    watch.standardOutput = NSFileHandle.fileHandleWithNullDevice;
    watch.standardError = NSFileHandle.fileHandleWithNullDevice;
    [watch launchAndReturnError:nil];
  }
}
/** Field-set + record post shared by mouse, wheel and keyboard events. */
static void cuPostEventRecord(cuBgLease *lease, CGEventRef e, uint32_t winNum, CGPoint winLoc) {
  CGEventSetIntegerValueField(e, 0, 3);
  CGEventSetIntegerValueField(e, 7, 3);
  CGEventSetIntegerValueField(e, 0x28, lease->targetPid);
  CGEventSetIntegerValueField(e, 0x33, winNum);
  CGEventSetIntegerValueField(e, 0x5b, winNum);
  CGEventSetIntegerValueField(e, 0x5c, winNum);
  cuSetWinLoc(e, winLoc);
  void *record = *(void **)((char *)e + 0x18);
  if(record) cuPostRecord(&lease->targetPSN, record); else CGEventPostToPid(lease->targetPid, e);
}
static id attr(AXUIElementRef el, NSString *name);
static void axPrepare(AXUIElementRef app);
/** The CGWindowNumber and frame of the layer-0 window of pid whose frame matches an AX window rect. */
static BOOL cuWindowNumberForFrame(pid_t pid, CGRect axFrame, uint32_t *outWin) {
  NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionAll, kCGNullWindowID));
  for(NSDictionary *w in windows) {
    if([w[(__bridge NSString *)kCGWindowOwnerPID] intValue] != pid) continue;
    if([w[(__bridge NSString *)kCGWindowLayer] intValue] != 0) continue;
    CGRect b;
    if(!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)w[(__bridge NSString *)kCGWindowBounds], &b)) continue;
    if(fabs(b.origin.x - axFrame.origin.x) > 2 || fabs(b.origin.y - axFrame.origin.y) > 2) continue;
    if(fabs(b.size.width - axFrame.size.width) > 2 || fabs(b.size.height - axFrame.size.height) > 2) continue;
    *outWin = [w[(__bridge NSString *)kCGWindowNumber] unsignedIntValue];
    return YES;
  }
  return NO;
}
static NSDictionary *cuBgPointer(NSRunningApplication *inputApp, NSDictionary *args) {
  if(!cuResolveBgPointer())
    @throw [NSException exceptionWithName:@"bg_dispatch_unavailable" reason:@"window-routed background pointer is unavailable: SLPSPostEventRecordTo/CGEventSetWindowLocation not resolvable via SkyLight; no input was sent" userInfo:nil];
  NSArray *steps = args[@"steps"];
  if(![steps isKindOfClass:NSArray.class] || !steps.count || steps.count > 400)
    @throw [NSException exceptionWithName:@"args" reason:@"bg_pointer needs 1..400 steps" userInfo:nil];
  CGPoint anchor = CGPointZero;
  BOOL haveAnchor = NO;
  for(NSDictionary *step in steps) {
    // A scroll step carries its point alongside the deltas.
    if([step[@"x"] isKindOfClass:NSNumber.class] && [step[@"y"] isKindOfClass:NSNumber.class]) {
      anchor = CGPointMake([step[@"x"] doubleValue], [step[@"y"] doubleValue]);
      haveAnchor = YES; break;
    }
  }
  if(!haveAnchor) @throw [NSException exceptionWithName:@"args" reason:@"bg_pointer steps carry no point" userInfo:nil];
  uint32_t winNum = 0;
  CGRect frame = CGRectZero;
  if(!cuWindowAtPointForPid(inputApp.processIdentifier, anchor, &winNum, &frame))
    @throw [NSException exceptionWithName:@"window" reason:@"no window of the bound application covers the start point; no input was sent" userInfo:nil];
  cuBgLease lease;
  NSString *why = nil;
  // Measured on macOS 26.1: view-level mouse delivery requires the window to
  // be key, and only the front-process lease makes it key. The window-focus
  // record alone makes it main — events reach the process and are swallowed.
  if(!cuBgLeaseBegin(inputApp, winNum, YES, &lease, &why))
    @throw [NSException exceptionWithName:@"bg_dispatch_unavailable" reason:why userInfo:nil];
  BOOL menuLeaseHeld = NO;
  BOOL restored = YES;
  @try {
    for(NSDictionary *step in steps) {
      cuCheckCancelled();
      if([step[@"scroll"] isKindOfClass:NSArray.class]) {
        NSArray *d = step[@"scroll"];
        // Pixel units: Chromium ignores line-unit wheel events entirely
        // (measured). One notch ≈ one line ≈ 40 px.
        CGEventRef wheel = CGEventCreateScrollWheelEvent(NULL, kCGScrollEventUnitPixel, 2, [d[1] intValue] * 40, [d[0] intValue] * 40);
        cuPostEventRecord(&lease, wheel, winNum, CGPointMake(anchor.x - frame.origin.x, anchor.y - frame.origin.y));
        CFRelease(wheel);
      } else {
        CGPoint p = CGPointMake([step[@"x"] doubleValue], [step[@"y"] doubleValue]);
        int type = [step[@"type"] intValue], button = [step[@"button"] intValue];
        CGEventRef e = CGEventCreateMouseEvent(NULL, (CGEventType)type, p, (CGMouseButton)button);
        CGEventSetIntegerValueField(e, kCGMouseEventClickState, [step[@"clickState"] longLongValue]);
        BOOL pressed = type == kCGEventLeftMouseDown || type == kCGEventRightMouseDown || type == kCGEventOtherMouseDown
                    || type == kCGEventLeftMouseDragged || type == kCGEventRightMouseDragged || type == kCGEventOtherMouseDragged;
        CGEventSetDoubleValueField(e, 2, pressed ? 1.0 : 0.0);
        cuPostEventRecord(&lease, e, winNum, CGPointMake(p.x - frame.origin.x, p.y - frame.origin.y));
        CFRelease(e);
      }
      usleep((useconds_t)([step[@"delayMs"] intValue] ?: 20) * 1000);
    }
    // A menu opened under a front lease dies when the lease ends; menus
    // animate in after the mouse-up, so a click-ending gesture polls briefly.
    // A web popup needs the poll to ride out Chromium's post-activation AX
    // rebuild (seconds), which the caller asks for with menu_poll_ms.
    if(lease.swapped) {
      int lastType = -1;
      for(NSDictionary *step in [steps reverseObjectEnumerator]) {
        if([step[@"type"] isKindOfClass:NSNumber.class]) { lastType = [step[@"type"] intValue]; break; }
      }
      BOOL clickEnded = lastType == kCGEventLeftMouseUp || lastType == kCGEventRightMouseUp || lastType == kCGEventOtherMouseUp;
      NSInteger pollMs = MAX(0, MIN(10000, [args[@"menu_poll_ms"] integerValue] ?: 720));
      menuLeaseHeld = cuMenuOpenForApp(inputApp.processIdentifier);
      if(!menuLeaseHeld && clickEnded) {
        for(NSInteger waited = 0; waited < pollMs && !menuLeaseHeld; waited += 60) {
          usleep(60000);
          menuLeaseHeld = cuMenuOpenForApp(inputApp.processIdentifier);
        }
      }
    }
  } @finally {
    if(menuLeaseHeld) cuFrontLeaseHold(&lease);
    else restored = cuBgLeaseEnd(&lease);
  }
  NSMutableDictionary *receipt = [@{@"action_sent":@YES, @"strategy":@"window-record", @"pointer_moved":@NO,
           @"front_lease":@(lease.swapped), @"window":@{@"id":@(winNum)}} mutableCopy];
  if(lease.swapped) receipt[@"front_restored"] = @(restored);
  cuLeaseAccounting(receipt, &lease);
  if(menuLeaseHeld) receipt[@"menu_lease_held"] = @YES;
  return receipt;
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
/**
 * Opt the target into full accessibility. Chromium-family apps (Chrome,
 * Electron) and WebKit content do not vend AXWebArea descendants until an
 * assistive client sets AXEnhancedUserInterface; Firefox-style engines gate
 * behind AXManualAccessibility. Without this an observe returns the chrome
 * of the window — menu bar, toolbar, tab strip — and no page content at all.
 * Apps that do not know these attributes simply refuse the write.
 */
static void axPrepare(AXUIElementRef app) {
#ifdef CU_TEST
  if([(__bridge id)app isKindOfClass:NSDictionary.class]) return;
#endif
  AXUIElementSetAttributeValue(app,(__bridge CFStringRef)@"AXEnhancedUserInterface",kCFBooleanTrue);
  AXUIElementSetAttributeValue(app,(__bridge CFStringRef)@"AXManualAccessibility",kCFBooleanTrue);
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
  // Web content nests deep: a browser form's controls commonly sit 12+ levels
  // under the window, and a real page holds hundreds of controls. Depth and
  // budget must cover that or every browser observe is silently headless.
  // A filtered observe (query/role) is a targeted search, not a page dump, so
  // it earns the deep budget — a control past the summary depth must still be
  // findable.
  BOOL deep=full||args[@"query"]||args[@"role"];
  NSInteger limit=deep?24:16, max=deep?1600:900;
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
/**
 * An AXWebArea whose recorded path produced no descendants is a page the walk
 * could not see into — typically Chromium still assembling its accessibility
 * subtree right after AXEnhancedUserInterface was set. Worth one re-observe
 * after a short settle rather than reporting an empty page.
 */
static BOOL hasOrphanWebArea(NSArray *out) {
  for(NSDictionary *d in out) {
    if(![d[@"role"] isEqual:@"AXWebArea"]) continue;
    NSArray *p=d[@"path"]; NSInteger w=[d[@"windowIndex"] integerValue];
    BOOL kids=NO;
    for(NSDictionary *e in out) {
      if(e==d || [e[@"windowIndex"] integerValue]!=w) continue;
      NSArray *q=e[@"path"];
      if(q.count<=p.count) continue;
      BOOL prefix=YES;
      for(NSUInteger i=0;i<p.count;i++) if(![q[i] isEqual:p[i]]) { prefix=NO; break; }
      if(prefix) { kids=YES; break; }
    }
    if(!kids) return YES;
  }
  return NO;
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
static BOOL axHasWebAncestor(AXUIElementRef el);
static NSString *cuClickAction(AXUIElementRef el, BOOL context) {
  id enabled=attr(el,@"AXEnabled");
  if([enabled isKindOfClass:NSNumber.class] && ![enabled boolValue]) return nil;
  NSArray *actions=cuActions(el);
  if(context) return [actions containsObject:@"AXShowMenu"]?@"AXShowMenu":nil;
  NSString *role=attr(el,@"AXRole");
  // A web popup button's AXPress does not open the native menu — only a real
  // mouse event does. Reporting it unpressable routes the click through the
  // window-record path, which opens the menu the page actually shows.
  if([@[@"AXMenuButton",@"AXPopUpButton"] containsObject:role] && axHasWebAncestor(el)) return nil;
  // Pressing a text field is toolkit-dependent; focus its insertion point directly.
  if([@[@"AXTextField",@"AXTextArea",@"AXComboBox"] containsObject:role] && cuSettable(el,@"AXFocused")) return @"AXFocused";
  if([actions containsObject:@"AXPress"]) return @"AXPress";
  if([role isEqual:@"AXMenuItem"] && [actions containsObject:@"AXPick"]) return @"AXPick";
  if([@[@"AXRow",@"AXCell"] containsObject:role] && cuSettable(el,@"AXSelected")) return @"AXSelected";
  // Focusing is the missing middle between "not pressable" and a real click,
  // but only for explicit text-entry roles. Anything else — a span, a group,
  // Chromium's page-content container, Qt composers — is better served by the
  // window-record click the caller falls back to: focusing a container is not
  // a click, and reporting one would swallow the press.
  if(cuSettable(el,@"AXFocused") && [@[@"AXTextField",@"AXTextArea",@"AXComboBox",@"AXSearchField",@"AXSecureTextField"] containsObject:role]) return @"AXFocused";
  return nil;
}
static NSDictionary *cuClick(AXUIElementRef el, BOOL context) {
  // A control whose rendered frame is empty is one a user could not click:
  // virtualized lists and collapsed regions vend elements that do not exist on
  // screen. Pressing one either does nothing or toggles a row the caller
  // cannot see. Refuse with the recovery spelled out.
  NSDictionary *sz=geometry(attr(el,@"AXSize"),YES);
  if(sz && ([sz[@"w"] doubleValue]<=0 || [sz[@"h"] doubleValue]<=0))
    @throw [NSException exceptionWithName:@"degenerate_frame" reason:@"target element has a degenerate frame (zero size); it is hidden or collapsed in a virtualized container — scroll it into view and observe again before clicking" userInfo:nil];
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
 * background helper never is. The WindowServer's own front-process channel
 * (0x200 = raising) works from any TCC-trusted process; setting AXFrontmost
 * through the Accessibility grant is the fallback.
 */
static BOOL axActivate(pid_t pid) {
  if(cuResolveBgPointer()) {
    ProcessSerialNumber psn;
    if(cuGetPSN(pid,&psn)==0 && cuSetFront(&psn,0,0x200)==0) return YES;
  }
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
/**
 * Whether an element lives inside a browser/webview subtree. Chromium accepts
 * AXSelectedText and AXValue writes on web controls and then ignores them —
 * or, worse, a numeric control coerces the write to empty. Web elements get
 * real keystrokes (after AXFocused) instead of semantic writes.
 */
static BOOL axHasWebAncestor(AXUIElementRef el) {
  // `node` stays an `id` so ARC keeps each ancestor alive through the walk —
  // a raw AXUIElementRef would dangle the moment `parent` is reassigned.
  id node=(__bridge id)el;
  for(int depth=0;node && depth<64;depth++) {
    NSString *role=attr((__bridge AXUIElementRef)node,@"AXRole");
    if([role isEqual:@"AXWebArea"]) return YES;
    id parent=attr((__bridge AXUIElementRef)node,@"AXParent");
    if(!parent || CFEqual((CFTypeRef)parent,(CFTypeRef)node)) break;
    node=parent;
  }
  return NO;
}
// Without a readable selection range only an exact append can be verified.
// Matching length or an already-present suffix is not evidence of delivery.
static BOOL cuTypeVerified(NSString *before, NSString *after, NSString *text) {
  return before && after && [after isEqual:[before stringByAppendingString:text]];
}
static id cuFocusedElement(pid_t pid) {
  AXUIElementRef appEl=AXUIElementCreateApplication(pid);
  AXUIElementSetMessagingTimeout(appEl,2.0);
  axPrepare(appEl);
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
  BOOL semantic=!simulated && focused && ![args[@"foreground_input"] boolValue] && cuSettable((__bridge AXUIElementRef)focused,@"AXSelectedText")
    && !axHasWebAncestor((__bridge AXUIElementRef)focused);
  if(semantic) {
    cuCheckCancelled();
    AXError error=AXUIElementSetAttributeValue((__bridge AXUIElementRef)focused,kAXSelectedTextAttribute,(__bridge CFStringRef)text);
    if(error!=kAXErrorSuccess) @throw [NSException exceptionWithName:@"action" reason:[NSString stringWithFormat:@"accessibility text insertion failed: %d; observe before retrying; no keyboard fallback was sent",error] userInfo:nil];
  }
  // One grapheme per event, the way a keyboard delivers them. Batching
  // several into one CGEventKeyboardSetUnicodeString is faster but Electron
  // apps coalesce the pending payload and keep only the final batch, so a
  // typed string silently arrives truncated to its tail.
  //
  // Astral graphemes (surrogate pairs, emoji, flags, ZWJ sequences) are
  // dropped by the WindowServer's key translation whenever the target window
  // is not front-and-visible — measured: occluded Chrome keeps every BMP
  // grapheme and loses 🐳. The window-record channel delivers the real event
  // instead. The trigger is the text, not an occlusion guess: if the string
  // carries any multi-unit grapheme, the whole stream rides the record
  // channel under one lease; a refused lease falls back to process posting.
  uint32_t typeWin = 0;
  CGRect typeFrame = CGRectZero;
  BOOL needsRecord = NO;
  if(!simulated && !semantic && focused && ![args[@"foreground_input"] boolValue] && cuResolveBgPointer()) {
    for(NSUInteger i = 0; i < text.length && !needsRecord;) {
      NSRange r = [text rangeOfComposedCharacterSequencesForRange:NSMakeRange(i, 1)];
      if(r.length > 1) needsRecord = YES;
      i = NSMaxRange(r);
    }
  }
  if(needsRecord) {
    id node = focused;
    for(int depth = 0; node && depth < 64; depth++) {
      if([attr((__bridge AXUIElementRef)node, @"AXRole") isEqual:@"AXWindow"]) {
        cuFrame((__bridge AXUIElementRef)node, &typeFrame)
          && cuWindowNumberForFrame(inputApp.processIdentifier, typeFrame, &typeWin);
        break;
      }
      id parent = attr((__bridge AXUIElementRef)node, @"AXParent");
      if(!parent || CFEqual((__bridge CFTypeRef)parent, (__bridge CFTypeRef)node)) break;
      node = parent;
    }
  }
  cuBgLease lease;
  BOOL leasing = NO;
  BOOL typeRestored = YES;
  if(needsRecord && typeWin) {
    NSString *why = nil;
    leasing = cuBgLeaseBegin(inputApp, typeWin, YES, &lease, &why);
  }
  @try {
    for(NSUInteger i=0;!semantic && i<text.length && !cuCancelled;) {
      if([args[@"foreground_input"] boolValue]) cuRequireForeground(inputApp);
      cuCheckCancelled();
      NSRange range=[text rangeOfComposedCharacterSequencesForRange:NSMakeRange(i,1)];
      NSString *chunk=[text substringWithRange:range];
      if(!simulated) for(int down=1;down>=0;down--) {
        CGEventRef event=textEvent(chunk,down);
        if([args[@"foreground_input"] boolValue]) CGEventPost(kCGHIDEventTap,event);
        else if(leasing) cuPostEventRecord(&lease, event, typeWin, CGPointMake(CGRectGetMidX(typeFrame) - typeFrame.origin.x, CGRectGetMidY(typeFrame) - typeFrame.origin.y));
        else CGEventPostToPid(inputApp.processIdentifier,event);
        CFRelease(event);
      }
      i=NSMaxRange(range); usleep(10000);
    }
  } @finally { if(leasing) typeRestored = cuBgLeaseEnd(&lease); }
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
                                  @"keyboard_delivery":semantic?@"accessibility":[args[@"foreground_input"] boolValue]?@"foreground-guarded":leasing?@"window-record":@"process",
                                  @"verified":@(verified),@"focused_role":role?:[NSNull null]} mutableCopy];
  if(leasing) receipt[@"window_focused"]=@YES;
  if(lease.swapped) receipt[@"front_restored"]=@(typeRestored);
  cuLeaseAccounting(receipt,&lease);
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
  if([tool isEqual:@"pointer_sequence"] && ![args[@"foreground_input"] boolValue] && ![args[@"app_scoped"] boolValue])
    @throw [NSException exceptionWithName:@"shared_pointer_required" reason:@"shared macOS pointer input is unavailable in background mode; use an accessibility action, strategy 'app' for a click inside the bound window, or a separate computer" userInfo:nil];
  cuOwnerPipe=[args[@"owner_pipe"] boolValue];
  BOOL mutates=[@[@"type",@"key_event",@"bg_key",@"mouse_event",@"scroll",@"pointer_sequence",@"bg_pointer",@"release_input",@"set_value",@"focus_element",@"select_text",@"perform_action",@"click_element",@"scroll_element"] containsObject:tool]
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
  if([tool isEqual:@"input_capabilities"]) return @{@"input_lease":@1,@"owner_pipe":@YES,@"record_owner_pipe":@1,@"window_ocr":@1,@"element_identity":@1,@"background_actions":@1,@"window_record":@(cuResolveBgPointer()?1:0)};
  if([tool isEqual:@"front_lease_watchdog"]) {
    long long remaining = [args[@"deadline"] longLongValue] - (long long)([NSDate new].timeIntervalSince1970 * 1000);
    if(remaining > 0 && remaining < 30000) usleep((useconds_t)remaining * 1000);
    NSData *data = [NSData dataWithContentsOfFile:cuFrontLeaseFile()];
    NSDictionary *held = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
    if([held isKindOfClass:NSDictionary.class] && [held[@"token"] isEqual:args[@"token"]]) cuFrontLeaseRestoreIfHeld();
    return @{@"ok":@YES};
  }
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
    for(NSRunningApplication *a in NSWorkspace.sharedWorkspace.runningApplications) {
      NSInteger policy = a.activationPolicy;
      NSString *policyName = (policy >= 0 && policy <= 2) ? @[@"regular",@"accessory",@"prohibited"][policy] : @"unknown";
      [apps addObject:@{@"name":a.localizedName?:@"",@"pid":@(a.processIdentifier),@"bundle_id":a.bundleIdentifier?:@"",@"frontmost":@(a.active),@"hidden":@(a.hidden),@"activation_policy":policyName}];
    }
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
    axPrepare(ax);
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
    if([a.bundleIdentifier isEqual:@"net.codewhale.computer-use"] && [args[@"activate"] boolValue]) @throw [NSException exceptionWithName:@"protected" reason:@"Computer Use safety controls belong to the user." userInfo:nil];
    if([args[@"activate"] boolValue]) cuLockInput();
    cuCheckCancelled();
    if([args[@"activate"] boolValue] && !axActivate(a.processIdentifier)) [a activateWithOptions:0];
    if([args[@"activate"] boolValue]) for(int i=0;i<120;i++) {
      cuCheckCancelled();
      // NSWorkspace only refreshes its frontmost view through run-loop
      // notifications; a one-shot helper that never services them reads a
      // stale answer for seconds and misreports a working activation.
      [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.025]];
      if(NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier==a.processIdentifier) break;
    }
    return @{@"found":@YES,@"name":a.localizedName?:@"",@"pid":@(a.processIdentifier),@"bundle_id":a.bundleIdentifier?:@"",@"frontmost":@(a.active)};
  }
  if([tool isEqual:@"kill_app"]) {
    NSString *bundle=args[@"bundle_id"], *name=args[@"name"]; NSNumber *pidNum=args[@"pid"];
    if(!bundle && !name && !pidNum) @throw [NSException exceptionWithName:@"args" reason:@"kill_app needs name, bundle_id or pid" userInfo:nil];
    if(pidNum && (![pidNum isKindOfClass:NSNumber.class] || [pidNum doubleValue]<=0 || [pidNum doubleValue]>INT_MAX || [pidNum doubleValue]!=[pidNum intValue])) @throw [NSException exceptionWithName:@"args" reason:@"kill_app pid must be a positive integer" userInfo:nil];
    // A name that matches two running apps must not guess which one to end.
    NSMutableArray *hits=[NSMutableArray array];
    for(NSRunningApplication *a in NSWorkspace.sharedWorkspace.runningApplications) {
      if(pidNum && a.processIdentifier!=[pidNum intValue]) continue;
      if(bundle && !matchesName(a.bundleIdentifier?:@"",bundle)) continue;
      if(name && !matchesName(a.localizedName?:@"",name)) continue;
      [hits addObject:a];
    }
    if(!hits.count) @throw [NSException exceptionWithName:@"app" reason:@"application not found" userInfo:nil];
    if(hits.count>1) {
      NSMutableArray *desc=[NSMutableArray array];
      for(NSRunningApplication *a in hits) [desc addObject:[NSString stringWithFormat:@"%@ (pid %d)",a.localizedName?:@"?",a.processIdentifier]];
      @throw [NSException exceptionWithName:@"app" reason:[NSString stringWithFormat:@"several running applications match (%@); pass pid to choose one",[desc componentsJoinedByString:@", "]] userInfo:nil];
    }
    NSRunningApplication *target=hits[0];
    pid_t tp=target.processIdentifier;
    // The helper, its host (the daemon or MCP server), and the app bundle that
    // owns this process must never be terminable through the agent surface.
    if([(target.bundleIdentifier?:@"") isEqual:@"net.codewhale.computer-use"] || tp==getpid() || tp==getppid())
      @throw [NSException exceptionWithName:@"protected" reason:@"the Computer Use helper and its host cannot be terminated by this plugin" userInfo:nil];
    [target terminate];
    for(int i=0;i<60 && !target.isTerminated;i++) { cuCheckCancelled(); [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]]; }
    BOOL forced=NO;
    if(!target.isTerminated && [args[@"force"] boolValue]) {
      [target forceTerminate]; forced=YES;
      for(int i=0;i<40 && !target.isTerminated;i++) { cuCheckCancelled(); [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]]; }
    }
    return @{@"killed":@(target.isTerminated),@"pid":@(tp),@"name":target.localizedName?:@"",@"force_used":@(forced)};
  }
  if([tool isEqual:@"installed_apps"]) {
    // Installed catalog: the apps a person could open, running or not. Root +
    // one level of subdirectories (e.g. /Applications/Utilities); bundle
    // identity comes from the bundle itself, never from the directory name.
    NSMutableDictionary *running=@{}.mutableCopy;
    for(NSRunningApplication *a in NSWorkspace.sharedWorkspace.runningApplications) {
      if(a.bundleIdentifier) running[a.bundleIdentifier]=@(a.processIdentifier);
    }
    NSMutableDictionary *byId=@{}.mutableCopy;
    NSFileManager *fm=NSFileManager.defaultManager;
    NSArray *roots=@[@"/Applications", @"/System/Applications", [NSHomeDirectory() stringByAppendingPathComponent:@"Applications"]];
    for(NSString *root in roots) {
      cuCheckCancelled();
      NSString *top=[root stringByResolvingSymlinksInPath];
      NSMutableArray *dirs=[NSMutableArray array]; [dirs addObject:top];
      for(NSString *sub in ([fm contentsOfDirectoryAtPath:top error:nil]?:@[])) {
        if([sub hasPrefix:@"."]||[sub hasSuffix:@".app"]) continue;
        NSString *p=[top stringByAppendingPathComponent:sub];
        BOOL isDir=NO;
        if([fm fileExistsAtPath:p isDirectory:&isDir] && isDir) [dirs addObject:p];
      }
      for(NSString *dir in dirs) {
        for(NSString *item in ([fm contentsOfDirectoryAtPath:dir error:nil]?:@[])) {
          if(![item hasSuffix:@".app"]) continue;
          NSString *p=[dir stringByAppendingPathComponent:item];
          NSBundle *b=[NSBundle bundleWithPath:p];
          NSString *bid=b.bundleIdentifier;
          if(!bid || byId[bid]) continue;
          NSString *name=[b objectForInfoDictionaryKey:@"CFBundleDisplayName"];
          if(!name.length) name=[b objectForInfoDictionaryKey:@"CFBundleName"];
          if(!name.length) name=[item stringByDeletingPathExtension];
          byId[bid]=@{@"name":name,@"bundle_id":bid,@"path":p}; 
        }
      }
    }
    NSMutableArray *out=[NSMutableArray array];
    for(NSString *bid in byId) {
      NSMutableDictionary *e=[byId[bid] mutableCopy];
      NSNumber *pid=running[bid];
      e[@"running"]=(pid!=nil)?@YES:@NO;
      if(pid) e[@"pid"]=pid;
      [out addObject:e];
    }
    [out sortUsingComparator:^NSComparisonResult(NSDictionary *a, NSDictionary *b){ return [a[@"name"] localizedCaseInsensitiveCompare:b[@"name"]]; }];
    return @{@"apps":out,@"count":@(out.count)};
  }
  if([tool isEqual:@"set_window_frame"]) {
    NSRunningApplication *a=resolve(args[@"app_ref"]);
    if(!a) @throw [NSException exceptionWithName:@"app" reason:@"application not found" userInfo:nil];
    NSDictionary *frame=args[@"frame"];
    double fx=NAN,fy=NAN,fw=NAN,fh=NAN;
    if([frame isKindOfClass:NSDictionary.class]) {
      fx=[frame[@"x"] doubleValue]; fy=[frame[@"y"] doubleValue];
      fw=[frame[@"w"] doubleValue]; fh=[frame[@"h"] doubleValue];
    }
    if(!isfinite(fx)||!isfinite(fy)||!isfinite(fw)||!isfinite(fh)||fw<=0||fh<=0)
      @throw [NSException exceptionWithName:@"args" reason:@"set_window_frame needs frame {x,y,w,h} with positive w/h" userInfo:nil];
    NSNumber *idxNum=args[@"window_id"];
    if(![idxNum isKindOfClass:NSNumber.class] || [idxNum doubleValue]!=[idxNum intValue] || [idxNum intValue]<0)
      @throw [NSException exceptionWithName:@"args" reason:@"set_window_frame needs window_id (a non-negative window index from list_windows)" userInfo:nil];
    AXUIElementRef app=AXUIElementCreateApplication(a.processIdentifier);
    axPrepare(app);
    NSArray *windows=attr(app,@"AXWindows");
    NSInteger idx=[idxNum intValue];
    if(idx>=(NSInteger)windows.count) { CFRelease(app); @throw [NSException exceptionWithName:@"window" reason:@"window_id is out of range; call list_windows for valid indices" userInfo:nil]; }
    AXUIElementRef win=(__bridge AXUIElementRef)windows[idx];
    CGRect before=CGRectNull; cuFrame(win,&before);
    cuCheckCancelled();
    CGPoint p=CGPointMake(fx,fy); CGSize z=CGSizeMake(fw,fh);
    AXValueRef pos=AXValueCreate(kAXValueCGPointType,&p), size=AXValueCreate(kAXValueCGSizeType,&z);
    AXError pe=AXUIElementSetAttributeValue(win,(__bridge CFStringRef)@"AXPosition",pos);
    AXError se=AXUIElementSetAttributeValue(win,(__bridge CFStringRef)@"AXSize",size);
    // Some apps re-anchor a window's origin when its size changes; re-assert
    // the position once after the size has had a run-loop turn to settle.
    if(pe==kAXErrorSuccess) {
      [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
      AXError pe2=AXUIElementSetAttributeValue(win,(__bridge CFStringRef)@"AXPosition",pos);
      if(pe2!=kAXErrorSuccess) pe=pe2;
    }
    if(pos) CFRelease(pos); if(size) CFRelease(size);
    if(pe!=kAXErrorSuccess && se!=kAXErrorSuccess) {
      CFRelease(app);
      @throw [NSException exceptionWithName:@"window" reason:@"the app refused the window frame change (it may be fullscreen, tiled or non-resizable)" userInfo:nil];
    }
    // Apps apply frame changes over a few run-loop turns; verify by reading the
    // window's own geometry back, not by trusting the set call.
    CGRect after=before;
    for(int i=0;i<40;i++) {
      cuCheckCancelled();
      [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
      cuFrame(win,&after);
      if(fabs(after.origin.x-fx)<1 && fabs(after.origin.y-fy)<1 && fabs(after.size.width-fw)<1 && fabs(after.size.height-fh)<1) break;
    }
    CFRelease(app);
    BOOL verified = fabs(after.origin.x-fx)<1 && fabs(after.origin.y-fy)<1 && fabs(after.size.width-fw)<1 && fabs(after.size.height-fh)<1;
    NSMutableDictionary *done=[@{@"action_sent":@YES,@"window_id":@(idx),
             @"before":@{@"x":@(before.origin.x),@"y":@(before.origin.y),@"w":@(before.size.width),@"h":@(before.size.height)},
             @"after":@{@"x":@(after.origin.x),@"y":@(after.origin.y),@"w":@(after.size.width),@"h":@(after.size.height)},
             @"verified":@(verified)} mutableCopy];
    if(pe!=kAXErrorSuccess || se!=kAXErrorSuccess) {
      done[@"ax_errors"]=@{@"position":@(pe),@"size":@(se)};
      done[@"note"]=@"the app constrained or refused part of the frame (minimum sizes and fixed-size windows are common); the after readback is what actually happened";
    }
    return done;
  }
  NSRunningApplication *inputApp=nil;
  if([@[@"type",@"key_event",@"bg_key",@"mouse_event",@"scroll",@"hit_test",@"pointer_sequence",@"bg_pointer"] containsObject:tool]) {
    if(![args[@"input_app_ref"] isKindOfClass:NSDictionary.class]) @throw [NSException exceptionWithName:@"focus" reason:@"open_application first to bind the input destination" userInfo:nil];
    inputApp=resolve(args[@"input_app_ref"]);
    if(!inputApp || inputApp.terminated) @throw [NSException exceptionWithName:@"focus" reason:@"input application is no longer running; open_application again" userInfo:nil];
    if([inputApp.bundleIdentifier isEqual:@"net.codewhale.computer-use"]) @throw [NSException exceptionWithName:@"protected" reason:@"Computer Use safety controls belong to the user." userInfo:nil];
  }
  // A held menu lease is given back before fresh raw input or an explicit
  // activation; AX element actions (the pick itself) leave it alone.
  if([@[@"bg_pointer",@"type",@"key_event",@"bg_key",@"pointer_sequence"] containsObject:tool]
     || ([tool isEqual:@"app_info"] && [args[@"activate"] boolValue]))
    cuFrontLeaseRestoreIfHeld();
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
  // A key chord with modifier flags through the window-record channel: menu
  // key equivalents (cmd+a, cmd+shift+g) only validate against a key window,
  // which the lease provides. Used by background select-all/replace flows.
  if([tool isEqual:@"bg_key"]) {
    if(!cuResolveBgPointer())
      @throw [NSException exceptionWithName:@"bg_dispatch_unavailable" reason:@"window-routed background keys are unavailable; no input was sent" userInfo:nil];
    uint32_t keyWin = 0;
    CGRect keyFrame = CGRectZero;
    id focused = cuFocusedElement(inputApp.processIdentifier);
    id node = focused;
    for(int depth = 0; node && depth < 64; depth++) {
      if([attr((__bridge AXUIElementRef)node, @"AXRole") isEqual:@"AXWindow"]) {
        cuFrame((__bridge AXUIElementRef)node, &keyFrame)
          && cuWindowNumberForFrame(inputApp.processIdentifier, keyFrame, &keyWin);
        break;
      }
      id parent = attr((__bridge AXUIElementRef)node, @"AXParent");
      if(!parent || CFEqual((__bridge CFTypeRef)parent, (__bridge CFTypeRef)node)) break;
      node = parent;
    }
    if(!keyWin) @throw [NSException exceptionWithName:@"focus" reason:@"no focused window for a window-routed key; focus a control first" userInfo:nil];
    cuBgLease lease;
    NSString *why = nil;
    if(!cuBgLeaseBegin(inputApp, keyWin, YES, &lease, &why))
      @throw [NSException exceptionWithName:@"bg_dispatch_unavailable" reason:why userInfo:nil];
    BOOL keyRestored = YES;
    @try {
      for(int down = 1; down >= 0; down--) {
        CGEventRef event = CGEventCreateKeyboardEvent(NULL, [args[@"code"] unsignedShortValue], down ? true : false);
        CGEventSetFlags(event, [args[@"flags"] unsignedLongLongValue]);
        cuPostEventRecord(&lease, event, keyWin, CGPointMake(CGRectGetMidX(keyFrame) - keyFrame.origin.x, CGRectGetMidY(keyFrame) - keyFrame.origin.y));
        CFRelease(event);
        usleep(30000);
      }
    } @finally { keyRestored = cuBgLeaseEnd(&lease); }
    NSMutableDictionary *receipt = [@{@"action_sent":@YES, @"strategy":@"window-record", @"keyboard_delivery":@"window-record", @"front_lease":@(lease.swapped)} mutableCopy];
    if(lease.swapped) receipt[@"front_restored"] = @(keyRestored);
    cuLeaseAccounting(receipt, &lease);
    return receipt;
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
    axPrepare(appEl);
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
    if(!chosen) {
      // A web popup button reports unpressable on purpose (its menu only opens
      // from a real mouse event); name it so the caller can poll for the menu
      // through Chromium's post-activation AX rebuild.
      if(hit) {
        for(id cur=hit; cur;) {
          AXUIElementRef el=(__bridge AXUIElementRef)cur;
          id role=attr(el,@"AXRole");
          if([role isEqual:@"AXWindow"] || [role isEqual:@"AXApplication"]) break;
          if([@[@"AXMenuButton",@"AXPopUpButton"] containsObject:role] && axHasWebAncestor(el))
            return @{@"found":@NO,@"reason":@"web_popup_requires_real_click"};
          cur=attr(el,@"AXParent");
        }
      }
      return @{@"found":@NO,@"reason":hit?@"no_pressable_element_at_point":@"no_element_at_point"};
    }

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
  if([tool isEqual:@"bg_pointer"]) return cuBgPointer(inputApp, args);
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
    // App-scoped clicks stay inside the bound window and do not steal the
    // foreground; they still move the real cursor and restore it.
    if([args[@"foreground_input"] boolValue]) cuRequireForeground(inputApp);
    // AppKit only assembles a drag out of events that look like they came from
    // the input hardware; a NULL-source stream delivers down and up but drops
    // every mouseDragged in between.
    CGEventSourceRef source=CGEventSourceCreate(kCGEventSourceStateHIDSystemState);
    BOOL held[3]={NO,NO,NO};
    CGPoint last=home;
    for(NSDictionary *step in args[@"steps"]) {
      @try { cuCheckCancelled(); if([args[@"foreground_input"] boolValue]) cuRequireForeground(inputApp); } @catch(NSException *e) { cuCancelled=1; break; }
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
  if(mutates && [a.bundleIdentifier isEqual:@"net.codewhale.computer-use"]) @throw [NSException exceptionWithName:@"protected" reason:@"Computer Use safety controls belong to the user." userInfo:nil];
  AXUIElementRef app=AXUIElementCreateApplication(a.processIdentifier);
  AXUIElementSetMessagingTimeout(app,2.0);
  axPrepare(app);
  @try {
    NSArray *ws=attr(app,@"AXWindows")?:@[];
    NSDictionary *identity=@{@"found":@YES,@"name":a.localizedName?:@"",@"pid":@(a.processIdentifier),@"bundle_id":a.bundleIdentifier?:@"",@"frontmost":@(a.active)};
    if([tool isEqual:@"get_app_state"] || [tool isEqual:@"list_windows"]) {
      BOOL truncated=NO;
      BOOL list=[tool isEqual:@"list_windows"];
      NSArray *out=observeElements(app,ws,args,list,&truncated);
      // A window that vends no descendants at all is not a page — it is an app
      // whose content tree is mid-rebuild (Chromium tears down and rebuilds
      // its accessibility tree across activation transitions, which takes
      // seconds). Poll briefly rather than returning an empty UI. A filtered
      // observe (query/role) legitimately matches nothing, so only unfiltered
      // observes earn the wait.
      // While a menu lease is held for this app, the thing being waited on is
      // the open menu's items: they reappear only when Chromium's rebuilt
      // tree re-vends them, so poll until an in-window AXMenuItem exists.
      BOOL leaseHeld = cuFrontLeaseHeldForPid(a.processIdentifier);
      int maxAttempts = leaseHeld ? 20 : 8;
      for(int attempt=0; !list && attempt<maxAttempts; attempt++) {
        BOOL empty = hasOrphanWebArea(out);
        if(!empty && !args[@"query"] && !args[@"role"]) {
          if(leaseHeld) {
            empty = YES;
            for(NSDictionary *d in out) {
              if([d[@"role"] isEqual:@"AXMenuItem"] && [d[@"windowIndex"] integerValue] >= 0) { empty = NO; break; }
            }
          } else if(ws.count) {
            BOOL anyContent = NO;
            for(NSDictionary *d in out) {
              if([d[@"windowIndex"] integerValue] >= 0 && [d[@"path"] count] > 0) { anyContent = YES; break; }
            }
            empty = !anyContent;
          }
        }
        if(!empty) break;
        usleep(300000);
        ws=attr(app,@"AXWindows")?:ws;
        truncated=NO;
        out=observeElements(app,ws,args,list,&truncated);
      }
      NSMutableDictionary *d=[identity mutableCopy]; d[list?@"windows":@"elements"]=out; d[@"truncated"]=@(truncated); return d;
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
    if([tool isEqual:@"get_value"]) {
      id v=attr((__bridge AXUIElementRef)el,@"AXValue");
      return @{@"ok":@YES,@"strategy":@"a11y",@"value":v?:[NSNull null],@"role":attr((__bridge AXUIElementRef)el,@"AXRole")?:[NSNull null]};
    }
    if([tool isEqual:@"set_value"]) {
      NSString *role=attr((__bridge AXUIElementRef)el,@"AXRole");
      if(axHasWebAncestor((__bridge AXUIElementRef)el))
        @throw [NSException exceptionWithName:@"value" reason:@"this element lives in a web area, which ignores background AXValue writes (numeric controls may even coerce them to empty). No write was sent — focus the element and use type instead, then verify with get_value." userInfo:nil];
      BOOL numeric=[@[@"AXIncrementor",@"AXSlider",@"AXStepper",@"AXValueIndicator",@"AXProgressIndicator"] containsObject:role];
      id value=args[@"value"];
      if(numeric) {
        // These controls type AXValue as a number. Writing a string is the
        // classic "clears the field instead of setting it" bug — a web
        // incrementor may coerce "" over "150" and report success.
        if(![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value)==CFBooleanGetTypeID()) {
          NSString *s=[value isKindOfClass:NSString.class]?value:[value description];
          static NSNumberFormatter *fmt=nil;
          if(!fmt) { fmt=[NSNumberFormatter new]; fmt.locale=[NSLocale localeWithLocaleIdentifier:@"en_US_POSIX"]; fmt.numberStyle=NSNumberFormatterDecimalStyle; }
          NSNumber *n=[fmt numberFromString:s];
          if(!n) @throw [NSException exceptionWithName:@"value" reason:[NSString stringWithFormat:@"%@ takes a numeric AXValue; %@ does not parse — focus the control and type instead",role?:@"this control",s] userInfo:nil];
          value=n;
        }
      }
      e=AXUIElementSetAttributeValue((__bridge AXUIElementRef)el,kAXValueAttribute,(__bridge CFTypeRef)value);
      if(e==kAXErrorSuccess) {
        usleep(60000);
        id after=attr((__bridge AXUIElementRef)el,@"AXValue");
        BOOL verified=NO;
        if(numeric) verified=[after isKindOfClass:NSNumber.class] && fabs([after doubleValue]-[value doubleValue])<1e-6;
        else verified=[after isKindOfClass:NSString.class] && [after isEqual:value];
        NSMutableDictionary *done=[@{@"action_sent":@YES,@"strategy":@"a11y",@"role":role?:[NSNull null],@"after":after?:[NSNull null],@"verified":@(verified)} mutableCopy];
        if(!verified) done[@"note"]=@"AXValue write did not verify: the control kept its own value (Electron/web text elements and numeric steppers commonly ignore background AXValue writes). Focus the element and use type instead, then verify with get_value.";
        return done;
      }
    }
    else if([tool isEqual:@"focus_element"]) e=AXUIElementSetAttributeValue((__bridge AXUIElementRef)el,kAXFocusedAttribute,kCFBooleanTrue);
    else if([tool isEqual:@"select_text"]){ NSArray *r=args[@"text_range"]?:@[@0,@0]; if(r.count!=2 || [r[0] longValue]<0 || [r[1] longValue]<0) @throw [NSException exceptionWithName:@"range" reason:@"text_range must be [start, length], both nonnegative" userInfo:nil]; CFRange range=CFRangeMake([r[0] longValue],[r[1] longValue]); AXValueRef v=AXValueCreate(kAXValueCFRangeType,&range); e=AXUIElementSetAttributeValue((__bridge AXUIElementRef)el,kAXSelectedTextRangeAttribute,v); CFRelease(v); }
    else if([tool isEqual:@"perform_action"]){ CFArrayRef actions=NULL; AXUIElementCopyActionNames((__bridge AXUIElementRef)el,&actions); NSArray *names=CFBridgingRelease(actions); if(![names containsObject:args[@"action"]]) @throw [NSException exceptionWithName:@"action" reason:@"action is not advertised by this element" userInfo:nil]; cuCheckCancelled(); e=AXUIElementPerformAction((__bridge AXUIElementRef)el,(__bridge CFStringRef)args[@"action"]); }
    if(e!=kAXErrorSuccess) @throw [NSException exceptionWithName:@"action" reason:[NSString stringWithFormat:@"accessibility action failed: %d",e] userInfo:nil];
    NSMutableDictionary *done=[@{@"action_sent":@YES,@"strategy":@"a11y"} mutableCopy];
    if([tool isEqual:@"focus_element"]) done[@"focused"]=@YES;
    return done;
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
