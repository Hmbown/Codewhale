param(
    [string]$Terminal,
    [string]$Binary,
    [string]$Workspace,
    [string]$BaseUrl,
    [string]$Title,
    [Parameter(Mandatory)][string]$Evidence,
    # One budget for every desktop stage. The Node caller derives its backstop
    # deadlines from this same number, so a stage always reports before Node
    # gives up.
    [int]$StageSeconds = 45,
    # Capture the desktop and the window list under this name, then exit. The
    # Node caller uses it when this script is stuck and cannot report itself.
    [string]$DiagnoseOnly = ''
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing
# Windows PowerShell 5.1 compiles this with the C# 5 compiler: no newer syntax.
Add-Type -IgnoreWarnings @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
public sealed class ClipboardFixtureWindowInfo {
    public int ZOrder;
    public long Handle;
    public uint ProcessId;
    public bool Foreground;
    public bool Minimised;
    public string Bounds;
    public string ClassName;
    public string Title;
}
public static class ClipboardFixtureWindow {
    public delegate bool EnumWindowsCallback(IntPtr h, IntPtr state);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left; public int Top; public int Right; public int Bottom; }
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int command);
    [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint processId);
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint from, uint to, bool attach);
    [DllImport("user32.dll")] static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindowsCallback callback, IntPtr state);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder text, int capacity);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder text, int capacity);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out Rect rect);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();

    static bool Owns(IntPtr h) { Thread.Sleep(60); return GetForegroundWindow() == h; }

    // Windows refuses SetForegroundWindow from a process that neither owns the
    // foreground nor sent the last input, which is the normal state of a CI
    // step. Escalate through the documented ways around that lock and name
    // the one that worked, so the evidence shows whether the lock was hit.
    // No step here sends anything to the owned Terminal: the Alt tap is only
    // issued while a different window still holds the foreground.
    public static string Activate(IntPtr h, bool allowMinimise) {
        if (GetForegroundWindow() == h) return "already";
        ShowWindow(h, IsIconic(h) ? 9 : 5);
        BringWindowToTop(h);
        SetForegroundWindow(h);
        if (Owns(h)) return "SetForegroundWindow";
        IntPtr other = GetForegroundWindow();
        uint otherProcess;
        uint otherThread = other == IntPtr.Zero ? 0 : GetWindowThreadProcessId(other, out otherProcess);
        uint thisThread = GetCurrentThreadId();
        if (otherThread != 0 && otherThread != thisThread && AttachThreadInput(thisThread, otherThread, true)) {
            try { BringWindowToTop(h); SetForegroundWindow(h); }
            finally { AttachThreadInput(thisThread, otherThread, false); }
            if (Owns(h)) return "AttachThreadInput";
        }
        if (GetForegroundWindow() != h) {
            keybd_event(0x12, 0, 0, UIntPtr.Zero);
            keybd_event(0x12, 0, 2, UIntPtr.Zero);
            SetForegroundWindow(h);
        }
        if (Owns(h)) return "Alt+SetForegroundWindow";
        if (allowMinimise) {
            ShowWindow(h, 6);
            Thread.Sleep(150);
            ShowWindow(h, 9);
            SetForegroundWindow(h);
            if (Owns(h)) return "minimise+restore";
        }
        return "denied";
    }

    // Front-to-back visible top-level windows. Plain user32 calls only, so
    // this still answers when UI Automation is the thing that is stuck.
    public static ClipboardFixtureWindowInfo[] VisibleTopLevelWindows() {
        List<ClipboardFixtureWindowInfo> rows = new List<ClipboardFixtureWindowInfo>();
        IntPtr foreground = GetForegroundWindow();
        EnumWindows(delegate(IntPtr h, IntPtr state) {
            if (!IsWindowVisible(h)) return true;
            StringBuilder title = new StringBuilder(512);
            GetWindowText(h, title, title.Capacity);
            StringBuilder className = new StringBuilder(256);
            GetClassName(h, className, className.Capacity);
            uint processId;
            GetWindowThreadProcessId(h, out processId);
            Rect r;
            GetWindowRect(h, out r);
            ClipboardFixtureWindowInfo info = new ClipboardFixtureWindowInfo();
            info.ZOrder = rows.Count;
            info.Handle = h.ToInt64();
            info.ProcessId = processId;
            info.Foreground = h == foreground;
            info.Minimised = IsIconic(h);
            info.Bounds = r.Left + "," + r.Top + "," + r.Right + "," + r.Bottom;
            info.ClassName = className.ToString();
            info.Title = title.ToString();
            rows.Add(info);
            return true;
        }, IntPtr.Zero);
        return rows.ToArray();
    }
}
'@
$window = $null
$ownedPid = $null
$ownedHandle = [IntPtr]::Zero
$stage = 'starting'
function Add-EvidenceLine([string]$File, [string]$Text) {
    $line = '{0:o} {1}' -f [DateTime]::UtcNow, $Text
    [System.IO.File]::AppendAllText((Join-Path $Evidence $File), $line + "`r`n")
    [Console]::Out.WriteLine("[$File] $line")
}
# The last line of stages.log names where a failed or killed run was; the
# timestamps show where a slow run spent its time.
function Enter-Stage([string]$Name) {
    $script:stage = $Name
    Add-EvidenceLine 'stages.log' $Name
}
function Save-WindowList([string]$Name) {
    $rows = @(foreach ($info in [ClipboardFixtureWindow]::VisibleTopLevelWindows()) {
        $path = $null
        try { $path = (Get-Process -Id $info.ProcessId -ErrorAction Stop).Path } catch { }
        [ordered]@{
            zOrder = $info.ZOrder; foreground = $info.Foreground; minimised = $info.Minimised
            title = $info.Title; className = $info.ClassName; bounds = $info.Bounds
            processId = $info.ProcessId; processPath = $path
            ownedTerminal = [bool]($ownedPid -and $info.ProcessId -eq $ownedPid)
            fixtureTitle = [bool]($Title -and $info.Title.Contains($Title))
        }
    })
    ConvertTo-Json -InputObject $rows -Depth 4 | Set-Content -Encoding UTF8 (Join-Path $Evidence "$Name-windows.json")
}
function Save-Desktop([string]$Name) {
    $screen = [System.Windows.Forms.SystemInformation]::VirtualScreen
    if ($screen.Width -le 0 -or $screen.Height -le 0) { throw 'No visible desktop' }
    $bitmap = [System.Drawing.Bitmap]::new([int]$screen.Width, [int]$screen.Height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen([int]$screen.X, [int]$screen.Y, 0, 0, $bitmap.Size)
        $bitmap.Save((Join-Path $Evidence "$Name-desktop.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
# Whole desktop plus every visible top-level window. The two parts are
# independent so one failing cannot hide the other or the original error.
function Save-Diagnostics([string]$Name) {
    try { Save-WindowList $Name } catch { Write-Warning "window list ${Name}: $_" }
    try { Save-Desktop $Name } catch { Write-Warning "desktop capture ${Name}: $_" }
}
function Read-Terminal {
    $texts = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::IsTextPatternAvailableProperty, $true))
    return (@($texts | ForEach-Object {
        $_.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern).DocumentRange.GetText(-1)
    }) -join "`n")
}
function Wait-For([scriptblock]$Check, [string]$Label) {
    $deadline = [DateTime]::UtcNow.AddSeconds($StageSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (& $Check) { return }
        Start-Sleep -Milliseconds 200
    }
    throw "Deadline (${StageSeconds}s) waiting for $Label"
}
function Capture([string]$Name) {
    (Read-Terminal) | Set-Content -Encoding UTF8 (Join-Path $Evidence "$Name.txt")
    $r = $window.Current.BoundingRectangle
    if ($r.IsEmpty -or $r.Width -le 0 -or $r.Height -le 0) { throw 'Terminal has no visible window' }
    $bitmap = [System.Drawing.Bitmap]::new([int]$r.Width, [int]$r.Height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen([int]$r.X, [int]$r.Y, 0, 0, $bitmap.Size)
        $bitmap.Save((Join-Path $Evidence "$Name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
function Focus-OwnedWindow([string]$Reason) {
    Enter-Stage "focus $Reason"
    $process = Get-Process -Id $ownedPid
    if ($process.Path -ne $Terminal) { throw 'Terminal process identity changed' }
    $handle = [IntPtr]$window.Current.NativeWindowHandle
    $script:ownedHandle = $handle
    # Re-assert until the owned window holds the foreground on two consecutive
    # looks. Only focus is retried: the paste and the Enter are each sent once.
    $attempt = 0
    $deadline = [DateTime]::UtcNow.AddSeconds($StageSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $attempt++
        $method = [ClipboardFixtureWindow]::Activate($handle, ($attempt -ge 3))
        if ($attempt -le 5 -or $method -ne 'denied') { Add-EvidenceLine 'focus.log' "$Reason attempt=$attempt method=$method" }
        if ($method -ne 'denied') {
            Start-Sleep -Milliseconds 150
            if ([ClipboardFixtureWindow]::GetForegroundWindow() -eq $handle) { return }
        }
        Start-Sleep -Milliseconds 350
    }
    throw "Deadline (${StageSeconds}s) waiting for owned Terminal focus $Reason after $attempt activation attempts"
}
if ($DiagnoseOnly) {
    Save-Diagnostics $DiagnoseOnly
    exit 0
}
try {
    Enter-Stage 'launch'
    # All arguments are owned fixture paths; the fake key cannot reach a paid provider.
    foreach ($arg in @($Terminal, $Binary, $Workspace, $BaseUrl, $Title)) {
        if (-not $arg) { throw 'Missing fixture argument' }
        if ($arg.Contains('"')) { throw 'Unexpected quote in fixture argument' }
    }
    # What the runner desktop held before this fixture added its own window.
    Save-Diagnostics 'start'
    $arguments = "-w new new-tab --title `"$Title`" --suppressApplicationTitle --startingDirectory `"$Workspace`" `"$Binary`" --workspace `"$Workspace`" --no-project-config --fresh --provider openai --model gpt-4o-mini --api-key clipboard-fixture-only --base-url `"$BaseUrl`""
    Start-Process -FilePath $Terminal -ArgumentList $arguments | Out-Null
    Enter-Stage 'window'
    Wait-For {
        $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
            [System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
        foreach ($candidate in $windows) {
            if ($candidate.Current.Name.Contains($Title)) {
                $process = Get-Process -Id $candidate.Current.ProcessId
                if ($process.Path -eq $Terminal) {
                    $script:window = $candidate; $script:ownedPid = $process.Id; return $true
                }
            }
        }
        return $false
    } 'owned Windows Terminal window'
    Focus-OwnedWindow 'before composer'
    Enter-Stage 'composer'
    Wait-For { (Read-Terminal).Contains('Type a message') } 'Codewhale composer'
    Capture 'ready'
    Enter-Stage 'clipboard'
    $payload = [System.IO.File]::ReadAllText((Join-Path $Evidence 'expected.txt'), [System.Text.Encoding]::UTF8)
    [System.Windows.Forms.Clipboard]::SetText($payload)
    if ([System.Windows.Forms.Clipboard]::GetText() -ne $payload) { throw 'Clipboard readback mismatch' }
    Focus-OwnedWindow 'before paste'
    Enter-Stage 'paste'
    [System.Windows.Forms.SendKeys]::SendWait('^+v')
    $foregroundAfterPaste = [ClipboardFixtureWindow]::GetForegroundWindow() -eq $ownedHandle
    # Let every pasted line arrive before asking the independent HTTP observer
    # whether anything was submitted prematurely.
    Start-Sleep -Milliseconds 1800
    Capture 'pasted'
    @{ processId = $ownedPid; sessionId = (Get-Process -Id $ownedPid).SessionId; chars = $payload.Length; input = 'Windows clipboard + Ctrl+Shift+V'; foregroundAfterPaste = $foregroundAfterPaste } |
        ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $Evidence 'pasted.json')
    Enter-Stage 'await submit'
    Wait-For { Test-Path (Join-Path $Evidence 'submit') } 'independent no-submit assertion'
    Focus-OwnedWindow 'before Enter'
    Enter-Stage 'enter'
    [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    Enter-Stage 'reply'
    Wait-For { (Read-Terminal).Contains('cw-clipboard-ok') } 'actual provider reply'
    Capture 'completed'
    @{ enterCount = 1; renderedReply = 'cw-clipboard-ok' } | ConvertTo-Json |
        Set-Content -Encoding UTF8 (Join-Path $Evidence 'completed.json')
    Enter-Stage 'done'
} catch {
    $failure = $_
    # Stage and message first, then the desktop (no UI Automation involved),
    # then the Terminal text, which needs the UI Automation that may have failed.
    try {
        @{ stage = $stage; error = "$failure" } | ConvertTo-Json |
            Set-Content -Encoding UTF8 (Join-Path $Evidence 'failure.json')
    } catch { Write-Warning "failure record: $_" }
    Save-Diagnostics 'failure'
    if ($window) { try { Capture 'failure' } catch { Write-Warning $_ } }
    throw $failure
} finally {
    # Retire only the portable Terminal instance proven to belong to this fixture.
    if ($ownedPid) {
        $process = Get-Process -Id $ownedPid -ErrorAction SilentlyContinue
        if ($process -and $process.Path -eq $Terminal) { Stop-Process -Id $ownedPid -Force }
    }
}
