# Spike S3 external memory sampler for Windows (the spike has no unsafe code, docs/05 §4).
# Runs the app, and on every "RESULT phase label=L pid=N" line it samples N's Private Working Set
# (docs/03 §1; the Task Manager "Memory" column) from the performance counter
# "\Process V2(<exe>:<pid>)\Working Set - Private" and prints a "RESULT mem" line.
# After the "6_dialog_open_4s" phase it also closes the folder dialog (WM_CLOSE to every visible
# top-level window of the process except the app window) so no human click is needed.
param([string]$Exe, [string]$ArgLine)

Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text;
public static class W {
  public delegate bool Cb(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(Cb cb, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  public static int CloseDialogs(uint pid) {
    int n = 0;
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h)) {
        var sb = new StringBuilder(128); GetWindowText(h, sb, 128);
        if (sb.ToString() != "ferrum-spike-s3") { PostMessage(h, 0x10, IntPtr.Zero, IntPtr.Zero); n++; }
      }
      return true;
    }, IntPtr.Zero);
    return n;
  }
}
"@

$psi = New-Object System.Diagnostics.ProcessStartInfo $Exe, $ArgLine
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardInput = $true            # one line = "sampling done" ack (S3_ACK)
$psi.UseShellExecute = $false
$psi.EnvironmentVariables["S3_ACK"] = "1"
$p = [System.Diagnostics.Process]::Start($psi)
$name = [IO.Path]::GetFileNameWithoutExtension($Exe)
while (($line = $p.StandardOutput.ReadLine()) -ne $null) {
  Write-Output $line
  if ($line -match 'RESULT phase label=(\S+) pid=(\d+)') {
    $label = $Matches[1]; $procId = $Matches[2]
    $inst = "$name`:$procId"
    $priv = (Get-Counter "\Process V2($inst)\Working Set - Private").CounterSamples[0].CookedValue
    $ws = (Get-Counter "\Process V2($inst)\Working Set").CounterSamples[0].CookedValue
    Write-Output ("RESULT mem phase={0} primary_bytes={1} primary_mb={2:N2} rss_bytes={3}" -f $label, [int64]$priv, ($priv / 1MB), [int64]$ws)
    if ($label -eq '6_dialog_open_4s') {
      Write-Output ("RESULT dialog_close windows_closed={0}" -f [W]::CloseDialogs([uint32]$procId))
    }
    $p.StandardInput.WriteLine("ack")
    $p.StandardInput.Flush()
  }
}
$p.WaitForExit()
