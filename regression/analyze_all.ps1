<#
.SYNOPSIS
    25 baseline feed uzerinde MD JAR calistirip bizim sonuclarla karsilastirir.
    Aggregate pattern analizi + per-feed tablo iceren HTML raporu uretir.
#>

$ErrorActionPreference = "Stop"
$ScriptDir   = $PSScriptRoot
$RepoRoot    = Split-Path $ScriptDir -Parent
$BinaryPath  = Join-Path $RepoRoot "target\release\examples\regress.exe"
$JarPath     = Join-Path $ScriptDir "gtfs-validator-8.0.1-cli.jar"
$BaselineDir = Join-Path $ScriptDir "baseline"
$TmpDir      = Join-Path $ScriptDir "tmp"
$MdDir       = Join-Path $TmpDir "md_all"
$OutPath     = Join-Path $ScriptDir "analyze_all.html"
$Today       = "20260101"
$MaxSizeBytes = 30MB

New-Item -ItemType Directory -Force -Path $TmpDir  | Out-Null
New-Item -ItemType Directory -Force -Path $MdDir   | Out-Null

# MD kural -> bizim kural eslesme haritasi (bilinen eslesmeler)
$KnownMap = @{
    "trip_headsign_matches_intermediate_stop" = "TRP_020"
    "stop_without_stop_time"                  = "STP_020"
    "future_feed"                             = "FIN_016"
    "unknown_column"                          = "ARC_017"
    "fast_travel_between_consecutive_stops"   = "STM_012/STM_014"
    "fast_travel_between_far_stops"           = "STM_012/STM_014"
    "stops_match_shape_out_of_order"          = "SHP_017"
    "stop_too_far_from_shape"                 = "SHP_012"
    "too_fast_travel"                         = "STM_012/STM_014"
    "trip_not_served_by_block"                = "TRP_015"
    "block_trips_with_overlapping_stop_times" = "TRP_022"
    "missing_trip_edge"                       = "XFL_002"
    "route_color_contrast"                    = "RTS_008"
    "same_name_and_description_for_stop"      = "STP_031"
    "stop_has_too_many_matches_for_shape"      = "SHP_022"
    "decreasing_shape_distance"               = "SHP_005"
    "equal_shape_distance_traveled_for_shapes_error" = "SHP_023"
    "missing_calendar_and_calendar_date_files" = "ARC_008"
    "unused_shape"                            = "SHP_018"
    "big_gap_in_service"                      = "CAL_007"
    "unknown_file"                            = "ARC_007"
    "stop_without_zone_id"                    = "STP_033"
    "leading_or_trailing_whitespaces"         = "DQ_016"
    "service_extends_far_in_the_future"       = "CAL_016"
    "service_window_outside_feed_period"      = "CAL_019"
    "expired_calendar"                        = "CAL_013"
    "trip_coverage_not_active_for_next7_days" = "TRP_023"
    "route_short_and_long_name_equal"         = "RTS_009"
    "overlapping_frequency"                   = "FRQ_009"
    "duplicate_route_long_name_and_short_name" = "RTS_019"
    "missing_required_file"                   = "ARC_004"
    "wrong_parent_location_type"              = "STP_010"
    "location_without_parent_station"         = "STP_011"
    "mixed_case_recommended_field"            = "DQ_018/DQ_019"
    "future_calendar"                         = "CAL_017"
    "unsorted_stop_times"                     = "STM_036"
    "missing_recommended_field"               = "DQ_020"
    "route_short_name_too_long"               = "RTS_010"
    "service_has_no_active_day_of_the_week"   = "CAL_006"
    "same_route_and_agency_url"               = "RTS_020"
    "route_long_name_contains_short_name"     = "RTS_022"
    "stop_too_far_from_shape_using_user_distance" = "SHP_024"
    "equal_shape_distance_same_coordinates"   = "SHP_023"
    "trip_distance_exceeds_shape_distance"    = "SHP_025"
    "trip_distance_exceeds_shape_distance_below_threshold" = "SHP_025"
    "missing_feed_contact_email_and_url"      = "FIN_018"
    "feed_expiration_date7_days"              = "FIN_019"
    "missing_recommended_file"               = "ARC_020"
    "duplicate_key"                          = "DQ_021"
    "missing_required_column"               = "ARC_012"
}

# Hic karsiligi olmayan MD kurallari (bilinen eksikler)
$KnownMissing = @(
    "station_with_parent_station"   # location_type=1 istasyonunun parent_station'ı var — bizde kural yok
)

# â”€â”€ Feed listesi â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

$feeds = Get-ChildItem $BaselineDir -Filter "*.json" | Sort-Object Name | ForEach-Object {
    $j = Get-Content $_.FullName -Raw | ConvertFrom-Json
    [PSCustomObject]@{
        id       = $_.BaseName
        url      = $j.meta.url
        country  = $j.meta.country
        provider = $j.meta.provider
        ourTotal = [int]$j.notice_total
        ourByRule= $j.by_rule
    }
}

Write-Host "Toplam $($feeds.Count) feed isleniyor..." -ForegroundColor Cyan

# â”€â”€ Her feed icin indir + MD calistir â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

$results = @()
$i = 0

foreach ($feed in $feeds) {
    $i++
    Write-Host "[$i/$($feeds.Count)] $($feed.id)  -  $($feed.provider)" -ForegroundColor White

    $zipPath = Join-Path $TmpDir "$($feed.id).zip"
    $mdOut   = Join-Path $MdDir  $feed.id

    # Indir
    if (-not (Test-Path $zipPath)) {
        try {
            $head = Invoke-WebRequest -Uri $feed.url -Method Head -UseBasicParsing -TimeoutSec 15 -ErrorAction SilentlyContinue
            $cl = if ($head -and $head.Headers['Content-Length']) { [int64]$head.Headers['Content-Length'] } else { 0 }
            if ($cl -gt 0 -and $cl -gt $MaxSizeBytes) {
                Write-Host "  ATLANDI: $([math]::Round($cl/1MB,1)) MB" -ForegroundColor DarkYellow
                $results += [PSCustomObject]@{ feed=$feed; mdNotices=@(); skipped=$true }
                continue
            }
            Invoke-WebRequest -Uri $feed.url -OutFile $zipPath -UseBasicParsing -TimeoutSec 120
        } catch {
            Write-Host "  INDIRME HATASI" -ForegroundColor Red
            $results += [PSCustomObject]@{ feed=$feed; mdNotices=@(); skipped=$true }
            continue
        }
    } else {
        Write-Host "  ZIP mevcut." -ForegroundColor DarkGray
    }

    # MD calistir
    New-Item -ItemType Directory -Force -Path $mdOut | Out-Null
    $mdReportPath = Join-Path $mdOut "report.json"
    if (-not (Test-Path $mdReportPath)) {
        try {
            java -jar $JarPath --input $zipPath --output_base $mdOut -d 2026-01-01 2>&1 | Out-Null
        } catch { }
    }

    $mdNotices = @()
    if (Test-Path $mdReportPath) {
        $mdNotices = (Get-Content $mdReportPath -Raw | ConvertFrom-Json).notices
        $mdSum = ($mdNotices | Measure-Object totalNotices -Sum).Sum
        Write-Host "  MD: $($mdNotices.Count) kural tipi, $mdSum notice" -ForegroundColor Green
    } else {
        Write-Host "  MD raporu olusturulamadi" -ForegroundColor Red
    }

    $results += [PSCustomObject]@{ feed=$feed; mdNotices=$mdNotices; skipped=$false }
    Remove-Item $zipPath -Force -ErrorAction SilentlyContinue
}

# â”€â”€ Aggregate analiz â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

# MD kural frekans sayaci (kac feedde goruldu)
$mdRuleFreq  = @{}   # rule -> feed sayisi
$mdRuleTotal = @{}   # rule -> toplam notice
$mdOnlyRules = @{}   # carsiligi olmayan MD kurallari

foreach ($r in $results) {
    if ($r.skipped) { continue }
    foreach ($n in $r.mdNotices) {
        $code = $n.code
        $mdRuleFreq[$code]  = ($mdRuleFreq[$code]  -as [int]) + 1
        $mdRuleTotal[$code] = ($mdRuleTotal[$code] -as [int]) + [int]$n.totalNotices
        if (-not $KnownMap.ContainsKey($code)) {
            $mdOnlyRules[$code] = ($mdOnlyRules[$code] -as [int]) + 1
        }
    }
}

# Bizim kural frekans sayaci
$ourRuleFreq  = @{}
$ourRuleTotal = @{}
foreach ($r in $results) {
    if ($r.skipped) { continue }
    foreach ($prop in $r.feed.ourByRule.PSObject.Properties) {
        $ourRuleFreq[$prop.Name]  = ($ourRuleFreq[$prop.Name]  -as [int]) + 1
        $ourRuleTotal[$prop.Name] = ($ourRuleTotal[$prop.Name] -as [int]) + [int]$prop.Value
    }
}

# â”€â”€ HTML olustur â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

$generatedAt = Get-Date -Format "yyyy-MM-dd HH:mm"
$feedCount   = ($results | Where-Object { -not $_.skipped }).Count

# Renk
function SevBadge($sev) {
    $col = switch ($sev) {
        "ERROR"   { "#c0392b" } "WARNING" { "#e67e22" }
        "INFO"    { "#2980b9" } default   { "#888" }
    }
    "<span style='background:$col;color:#fff;border-radius:3px;padding:1px 6px;font-size:10px;font-weight:700'>$sev</span>"
}

# â”€â”€ 1. MD kural frekans tablosu â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

$mdFreqRows = ""
$mdRuleFreq.GetEnumerator() | Sort-Object { -[int]$_.Value } | ForEach-Object {
    $code  = $_.Key
    $freq  = [int]$_.Value
    $total = [int]$mdRuleTotal[$code]
    $our   = if ($KnownMap.ContainsKey($code)) { $KnownMap[$code] } else { "-" }
    $missing = $KnownMissing -contains $code
    $rowBg = if ($missing) { "background:#fff8f8" } elseif ($our -ne "-") { "background:#f0fff4" } else { "" }
    $ourCell = if ($our -ne "-") {
        "<span style='font-family:monospace;color:#1e8449;font-weight:600'>$our</span>"
    } else {
        "<span style='color:#e74c3c;font-weight:600'>EKSIK KURAL</span>"
    }
    $pct = [math]::Round($freq * 100.0 / $feedCount)
    $mdFreqRows += "<tr style='$rowBg'>
        <td style='font-family:monospace;font-size:12px'>$code</td>
        <td style='text-align:center;font-weight:600'>$freq / $feedCount</td>
        <td style='text-align:center'>
            <div style='background:#e0e0e0;border-radius:3px;height:6px;width:80px;display:inline-block;vertical-align:middle'>
                <div style='width:$([math]::Min($pct,100))%;height:100%;background:#2980b9;border-radius:3px'></div>
            </div>
        </td>
        <td style='text-align:right'>$total</td>
        <td>$ourCell</td>
    </tr>"
}

# â”€â”€ 2. Bizim kural frekans tablosu (top 20) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

$ourFreqRows = ""
$ourRuleFreq.GetEnumerator() | Sort-Object { -[int]$_.Value } | Select-Object -First 25 | ForEach-Object {
    $code  = $_.Key
    $freq  = [int]$_.Value
    $total = [int]$ourRuleTotal[$code]
    $pct   = [math]::Round($freq * 100.0 / $feedCount)
    $ourFreqRows += "<tr>
        <td style='font-family:monospace;font-size:12px;font-weight:600'>$code</td>
        <td style='text-align:center;font-weight:600'>$freq / $feedCount</td>
        <td style='text-align:center'>
            <div style='background:#e0e0e0;border-radius:3px;height:6px;width:80px;display:inline-block;vertical-align:middle'>
                <div style='width:$([math]::Min($pct,100))%;height:100%;background:#1e8449;border-radius:3px'></div>
            </div>
        </td>
        <td style='text-align:right'>$total</td>
    </tr>"
}

# â”€â”€ 3. Per-feed ozet tablosu â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

$perFeedRows = ""
foreach ($r in $results) {
    $f = $r.feed
    if ($r.skipped) {
        $perFeedRows += "<tr style='background:#fafafa'><td style='font-family:monospace'>$($f.id)</td><td>$($f.provider)</td>
            <td colspan='5' style='color:#999;font-style:italic'>AtlandÄ± (boyut limiti)</td></tr>"
        continue
    }
    $mdTotal  = ($r.mdNotices | Measure-Object totalNotices -Sum).Sum
    $mdTypes  = $r.mdNotices.Count
    $matches  = ($r.mdNotices | Where-Object { $KnownMap.ContainsKey($_.code) }).Count
    $missing  = ($r.mdNotices | Where-Object { $KnownMissing -contains $_.code }).Count
    $unknown  = $mdTypes - $matches - $missing

    $matchPct = if ($mdTypes -gt 0) { [math]::Round($matches * 100.0 / $mdTypes) } else { 100 }
    $barColor = if ($matchPct -ge 80) { "#1e8449" } elseif ($matchPct -ge 50) { "#e67e22" } else { "#c0392b" }

    # MD kural listesi (kisa)
    $mdList = ($r.mdNotices | Sort-Object totalNotices -Descending | Select-Object -First 5 |
        ForEach-Object { "$($_.code)($($_.totalNotices))" }) -join ", "

    $perFeedRows += "<tr>
        <td style='font-family:monospace;font-size:12px'>$($f.id)</td>
        <td style='font-size:12px'>$($f.provider)</td>
        <td style='text-align:right;font-weight:600'>$($f.ourTotal)</td>
        <td style='text-align:right'>$mdTotal</td>
        <td style='text-align:center'>
            <div style='background:#e0e0e0;border-radius:3px;height:8px;width:60px;display:inline-block;vertical-align:middle'>
                <div style='width:$matchPct%;height:100%;background:$barColor;border-radius:3px'></div>
            </div>
            <span style='font-size:11px;color:#666'> $matchPct%</span>
        </td>
        <td style='text-align:center;color:#c0392b;font-weight:600'>$missing</td>
        <td style='font-size:11px;color:#777;max-width:300px;word-break:break-all'>$mdList</td>
    </tr>"
}

# â”€â”€ 4. Eksik kural ozeti â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

$missingRuleRows = ""
$mdOnlyRules.GetEnumerator() | Sort-Object { -[int]$_.Value } | ForEach-Object {
    $code = $_.Key
    $freq = [int]$_.Value
    $total = [int]$mdRuleTotal[$code]
    $isKnownMissing = $KnownMissing -contains $code
    $label = if ($isKnownMissing) {
        "<span style='background:#fdecea;color:#c0392b;border-radius:4px;padding:1px 6px;font-size:10px'>Bilinen eksik</span>"
    } else {
        "<span style='background:#fef9e7;color:#b7950b;border-radius:4px;padding:1px 6px;font-size:10px'>Yeni</span>"
    }
    $missingRuleRows += "<tr>
        <td style='font-family:monospace;font-size:12px'>$code</td>
        <td style='text-align:center;font-weight:600'>$freq feed</td>
        <td style='text-align:right'>$total</td>
        <td>$label</td>
    </tr>"
}

# â”€â”€ HTML birlesim â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

$totalMdNotices = ($results | Where-Object { -not $_.skipped } |
    ForEach-Object { ($_.mdNotices | Measure-Object totalNotices -Sum).Sum } |
    Measure-Object -Sum).Sum
$totalOurNotices = ($results | Where-Object { -not $_.skipped } |
    ForEach-Object { $_.feed.ourTotal } | Measure-Object -Sum).Sum
$uniqueMdRules = $mdRuleFreq.Count
$missingCount  = $mdOnlyRules.Count

$html = @"
<!DOCTYPE html><html lang="tr"><head><meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>GTFS Analyzer - 25 Feed Aggregate Analiz</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;background:#f5f7fa;color:#222;font-size:14px}
table{width:100%;border-collapse:collapse}
th,td{padding:8px 12px;border-top:1px solid #f0f0f0;text-align:left;vertical-align:middle}
th{background:#f5f7fa;border-top:none;font-size:11px;text-transform:uppercase;letter-spacing:.4px;color:#666;white-space:nowrap}
tr:hover td{background:#fafbff}
h2{font-size:15px;font-weight:600;color:#333;margin:24px 0 10px}
.card{background:#fff;border-radius:8px;box-shadow:0 1px 4px rgba(0,0,0,.08);overflow:hidden;margin-bottom:20px}
.section{padding:20px 28px}
</style></head><body>

<div style="background:#1a1a2e;color:#fff;padding:20px 28px">
    <div style="font-size:11px;color:#aaa;margin-bottom:4px">Aggregate Analiz &mdash; $generatedAt</div>
    <h1 style="font-size:22px;font-weight:700;margin-bottom:4px">25 Feed Karsilastirmali Analiz</h1>
    <div style="color:#aaa;font-size:13px">Bizim Validator vs MobilityData v8.0.1 &nbsp;|&nbsp; Tarih: 2026-01-01 &nbsp;|&nbsp; $feedCount feed islendi</div>
</div>

<div style="display:flex;background:#fff;border-bottom:1px solid #eee;flex-wrap:wrap">
    <div style="flex:1;min-width:120px;text-align:center;padding:16px;border-right:1px solid #eee">
        <div style="font-size:26px;font-weight:700;color:#1a1a2e">$feedCount</div>
        <div style="font-size:11px;color:#888;text-transform:uppercase">Islenen Feed</div>
    </div>
    <div style="flex:1;min-width:120px;text-align:center;padding:16px;border-right:1px solid #eee">
        <div style="font-size:26px;font-weight:700;color:#1e8449">$totalOurNotices</div>
        <div style="font-size:11px;color:#888;text-transform:uppercase">Bizim Notice</div>
    </div>
    <div style="flex:1;min-width:120px;text-align:center;padding:16px;border-right:1px solid #eee">
        <div style="font-size:26px;font-weight:700;color:#2980b9">$totalMdNotices</div>
        <div style="font-size:11px;color:#888;text-transform:uppercase">MD Notice</div>
    </div>
    <div style="flex:1;min-width:120px;text-align:center;padding:16px;border-right:1px solid #eee">
        <div style="font-size:26px;font-weight:700;color:#8e44ad">$uniqueMdRules</div>
        <div style="font-size:11px;color:#888;text-transform:uppercase">Benzersiz MD Kurali</div>
    </div>
    <div style="flex:1;min-width:120px;text-align:center;padding:16px">
        <div style="font-size:26px;font-weight:700;color:#c0392b">$missingCount</div>
        <div style="font-size:11px;color:#888;text-transform:uppercase">Bizde Eksik MD Kurali</div>
    </div>
</div>

<div class="section">

<h2>MD Kural Frekansi (25 Feed Genelinde)</h2>
<p style="font-size:12px;color:#777;margin-bottom:10px">
    Yesil satir: bizde karsiligi var &nbsp;|&nbsp; Kirmizi satir: eksik kural &nbsp;|&nbsp; Beyaz: kismi/belirsiz eslesme
</p>
<div class="card"><table>
<thead><tr>
    <th>MD Kurali</th><th style="text-align:center">Kac Feedde</th><th style="text-align:center">Yayginlik</th>
    <th style="text-align:right">Toplam Notice</th><th>Bizim KarsÄ±lÄ±gÄ±</th>
</tr></thead>
<tbody>$mdFreqRows</tbody>
</table></div>

<h2>Bizim En Yaygin Kuralarimiz (25 Feed Genelinde)</h2>
<div class="card"><table>
<thead><tr>
    <th>Kural</th><th style="text-align:center">Kac Feedde</th><th style="text-align:center">Yayginlik</th>
    <th style="text-align:right">Toplam Notice</th>
</tr></thead>
<tbody>$ourFreqRows</tbody>
</table></div>

<h2>Bizde Karsiligi Olmayan MD Kurallari</h2>
<div class="card"><table>
<thead><tr>
    <th>MD Kurali</th><th style="text-align:center">Kac Feedde</th>
    <th style="text-align:right">Toplam Notice</th><th>Durum</th>
</tr></thead>
<tbody>$missingRuleRows</tbody>
</table></div>

<h2>Per-Feed Ozet</h2>
<div class="card"><table>
<thead><tr>
    <th>Feed</th><th>Saglayici</th>
    <th style="text-align:right">Bizim</th><th style="text-align:right">MD</th>
    <th style="text-align:center">Esleme</th>
    <th style="text-align:center">Eksik Kural</th>
    <th>MD Kurallari (top 5)</th>
</tr></thead>
<tbody>$perFeedRows</tbody>
</table></div>

</div>
</body></html>
"@

$html | Set-Content -Path $OutPath -Encoding utf8
Write-Host "`nRapor: $OutPath" -ForegroundColor Green
Start-Process $OutPath

