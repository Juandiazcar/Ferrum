# Ferrum shell integration for PowerShell 5.1 and 7 (FR-SESS-005): OSC 7 reports the working
# directory and OSC 133 marks prompt (A), input (B), output (C, with PSReadLine) and the exit
# status (D). Ferrum loads this file with `-NoExit -Command`, after the user's profile, and never
# edits it.

$global:__FerrumEsc = [char]27
$global:__FerrumBel = [char]7
$global:__FerrumUserPrompt = $function:prompt
$global:__FerrumPrompted = $false

function global:prompt {
    $succeeded = $?
    $exitCode = $global:LASTEXITCODE
    $e = $global:__FerrumEsc
    $b = $global:__FerrumBel
    $marks = ""
    if ($global:__FerrumPrompted) {
        $status = if ($succeeded) { 0 } elseif ($exitCode) { $exitCode } else { 1 }
        $marks += "$e]133;D;$status$b"
    }
    $global:__FerrumPrompted = $true
    if ($PWD.Provider.Name -eq "FileSystem") {
        $marks += "$e]7;$(([System.Uri]$PWD.ProviderPath).AbsoluteUri)$b"
    }
    $userPrompt = & $global:__FerrumUserPrompt
    $global:LASTEXITCODE = $exitCode
    "$marks$e]133;A$b$userPrompt$e]133;B$b"
}

if (Get-Module PSReadLine) {
    Set-PSReadLineKeyHandler -Chord Enter -ScriptBlock {
        [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
        [Console]::Write("$($global:__FerrumEsc)]133;C$($global:__FerrumBel)")
    }
}
