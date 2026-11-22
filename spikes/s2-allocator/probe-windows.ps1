# Prints the Private Working Set (bytes) of process <pid>, docs/03 §1 metric on Windows.
# Method: PDH counter '\Process(*)\Working Set - Private' (what Task Manager's "Memory" column
# reports), matched to the PID through '\Process(*)\ID Process' sampled in the same call.
# Fallback for non-English Windows, where counter names are localized: the same PDH data through
# WMI, Win32_PerfFormattedData_PerfProc_Process.WorkingSetPrivate. Prints "<bytes> <method>".
param([Parameter(Mandatory = $true)][int]$ProcId)
$ErrorActionPreference = 'Stop'
try {
    $s = (Get-Counter '\Process(*)\ID Process', '\Process(*)\Working Set - Private').CounterSamples
    $inst = ($s | Where-Object { $_.Path -like '*\id process' -and [int]$_.CookedValue -eq $ProcId }).InstanceName
    if (-not $inst) { throw "no instance for pid $ProcId" }
    $v = ($s | Where-Object { $_.Path -like '*\working set - private' -and $_.InstanceName -eq $inst }).CookedValue
    '{0} Get-Counter' -f [int64]$v
} catch {
    $p = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess=$ProcId"
    '{0} CIM-WorkingSetPrivate' -f [int64]$p.WorkingSetPrivate
}
