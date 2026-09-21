param([string]$Title, [string]$Receipt)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
$form = New-Object System.Windows.Forms.Form
$form.Text = $Title
$form.Width = 420
$form.Height = 220
$panel = New-Object System.Windows.Forms.Panel
$panel.Dock = 'Fill'
$edit = New-Object System.Windows.Forms.TextBox
$edit.Name = 'FixtureEntry'
$edit.Text = 'initial'
$edit.SetBounds(20, 20, 320, 30)
$button = New-Object System.Windows.Forms.Button
$button.Text = 'Apply fixture'
$button.SetBounds(20, 70, 160, 35)
$script:clicks = 0
$button.Add_Click({ $script:clicks++ })
$panel.Controls.Add($button)
$panel.Controls.Add($edit)
$form.Controls.Add($panel)
$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 100
$timer.Add_Tick({
  $json = @{ title = $form.Text; visible = $form.Visible; text = $edit.Text; clicks = $script:clicks; hwnd = $form.Handle.ToInt64() } | ConvertTo-Json -Compress
  [IO.File]::WriteAllText($Receipt + '.tmp', $json, (New-Object Text.UTF8Encoding($false)))
  Move-Item -Force ($Receipt + '.tmp') $Receipt
})
$timer.Start()
try { [System.Windows.Forms.Application]::Run($form) } finally { $timer.Dispose(); $form.Dispose() }
