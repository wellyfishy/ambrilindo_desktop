# Frees the Vite dev server port before `tauri dev` (used by run-desktop.bat).
# Only a leftover Vite dev server (node running vite) is stopped. Anything else on the
# port is reported and left alone, so another program is never killed by accident.
param(
    [int[]]$Ports = @(1420, 1421)
)

$blocked = $false
foreach ($port in $Ports) {
    $listeners = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue
    foreach ($procId in ($listeners | Select-Object -ExpandProperty OwningProcess -Unique)) {
        $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$procId" -ErrorAction SilentlyContinue
        if (-not $proc) { continue }
        $cmd = [string]$proc.CommandLine
        if ($proc.Name -ieq "node.exe" -and $cmd -match "vite") {
            Write-Host "      Port $port dipakai dev server Vite lama (PID $procId), dihentikan."
            try {
                Stop-Process -Id $procId -Force -ErrorAction Stop
            } catch {
                Write-Host "      Gagal menghentikan PID ${procId}: $($_.Exception.Message)"
                $blocked = $true
            }
        } else {
            Write-Host "      Port $port dipakai program lain: $($proc.Name) (PID $procId). Tidak dihentikan."
            $blocked = $true
        }
    }
}

if ($blocked) {
    Write-Host "      Tutup program tersebut lalu jalankan ulang run-desktop.bat."
    exit 1
}
# Give Windows a moment to release the socket
Start-Sleep -Milliseconds 500
exit 0
