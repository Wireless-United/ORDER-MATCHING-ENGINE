<#
Simple FIX TCP load tester for PowerShell
Usage:
  .\load_test_fix.ps1 -Host 127.0.0.1 -Port 8888 -Concurrency 10 -RequestsPerWorker 100 -Interval 0.1
#>
param(
    [string]$Host = "127.0.0.1",
    [int]$Port = 8888,
    [int]$Concurrency = 10,
    [int]$RequestsPerWorker = 100,
    [double]$Interval = 0.1,
    [string]$MessageFile = ""
)

if ($MessageFile -and (Test-Path $MessageFile)) {
    $FIX_MSG = Get-Content -Raw $MessageFile
} else {
    $FIX_MSG = "8=FIX.4.2|9=...|35=D|55=Pranesh|54=1|44=100|38=10|10=000|`n"
}

Write-Host "Load test: host=$Host port=$Port concurrency=$Concurrency requests_per_worker=$RequestsPerWorker interval=$Interval"

function Send-FixOnce {
    param($Host, $Port, $Message)
    try {
        $client = New-Object System.Net.Sockets.TcpClient
        $async = $client.BeginConnect($Host, $Port, $null, $null)
        $wait = $async.AsyncWaitHandle.WaitOne(2000)
        if (-not $wait) {
            $client.Close()
            return ""
        }
        $client.EndConnect($async)
        $stream = $client.GetStream()
        $writer = New-Object System.IO.StreamWriter($stream)
        $writer.AutoFlush = $true
        $writer.Write($Message)
        # read response with 2s timeout
        $stream.ReadTimeout = 2000
        $reader = New-Object System.IO.StreamReader($stream)
        $resp = ""
        try { $resp = $reader.ReadLine() } catch { $resp = "" }
        $writer.Close(); $reader.Close(); $client.Close()
        return $resp
    } catch {
        return ""
    }
}

$jobs = @()
for ($w=1; $w -le $Concurrency; $w++) {
    $job = Start-Job -ScriptBlock {
        param($id, $Host, $Port, $RequestsPerWorker, $Interval, $Message)
        for ($i=1; $i -le $RequestsPerWorker; $i++) {
            $resp = Send-FixOnce -Host $Host -Port $Port -Message $Message
            if ($resp) {
                Write-Output "[worker $id] ok: got $($resp.Length) chars"
            } else {
                Write-Output "[worker $id] ok: no response"
            }
            if ($i -lt $RequestsPerWorker) { Start-Sleep -Seconds $Interval }
        }
        Write-Output "[worker $id] finished"
    } -ArgumentList $w, $Host, $Port, $RequestsPerWorker, $Interval, $FIX_MSG
    $jobs += $job
}

# Wait for jobs
Receive-Job -Job $jobs -Wait -AutoRemoveJob | ForEach-Object { Write-Host $_ }

Write-Host "All workers finished."