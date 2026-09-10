<#
.SYNOPSIS
    精灵图色彩一致性自检 —— 量化各动画之间的色彩偏移。

.DESCRIPTION
    桌宠的动画素材是分批绘制的，不同批次容易出现饱和度 / 明度 / 冷暖的偏差。
    单看一张图不容易发现，但在动画切换的瞬间（尤其是跨批次的两个动作之间）
    会明显"闪"一下。

    本脚本逐 tag 统计实心像素（alpha > 阈值）的平均 R/G/B、饱和度、明度与冷暖比，
    并以指定基准 tag（默认 Idle）给出偏移量，输出可直接照着调色的对照表。

.PARAMETER Sheet
    精灵图 PNG 路径。省略时使用默认素材 public/pet/pet_spritesheet.png。

.PARAMETER Json
    Aseprite 导出的 JSON 路径。省略时使用 public/pet/pet_spritesheet.json。

.PARAMETER Baseline
    基准 tag 名（默认 Idle）。所有偏移量都是相对它计算的。

.PARAMETER AlphaThreshold
    视为"实心像素"的 alpha 下限（默认 200）。调低会把半透明边缘也算进来，
    使统计更容易被抗锯齿边缘影响。

.PARAMETER OutFile
    可选。把报告写入该文件（UTF-8）。省略则直接打印到控制台。

.EXAMPLE
    # 在 DeskPet 目录下运行，使用默认素材
    pwsh -File docs/tools/color-audit.ps1

.EXAMPLE
    # 指定自定义素材，并输出到文件
    pwsh -File docs/tools/color-audit.ps1 -Sheet .\my-skin.png -Json .\my-skin.json -OutFile report.txt

.NOTES
    局限：统计的是整帧平均值，因此"构图比例不同"（例如某动作露出更多白色衣料）
    也会影响结果，未必都是真正的配色偏差。明度类结论比饱和度类结论更可靠。
    建议把本脚本的输出当作"重点复查清单"，最终以肉眼在实机上确认。

    需要 Windows PowerShell 5.1+ / PowerShell 7+（依赖 System.Drawing）。
#>
[CmdletBinding()]
param(
    [string]$Sheet,
    [string]$Json,
    [string]$Baseline = 'Idle',
    [int]$AlphaThreshold = 200,
    [string]$OutFile
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

# 默认素材路径：脚本位于 <project>/docs/tools/，素材在 <project>/public/pet/
$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not $Sheet) { $Sheet = Join-Path $projectRoot 'public\pet\pet_spritesheet.png' }
if (-not $Json)  { $Json  = Join-Path $projectRoot 'public\pet\pet_spritesheet.json' }

foreach ($p in @($Sheet, $Json)) {
    if (-not (Test-Path $p)) { throw "找不到文件: $p" }
}
if (-not (Test-Path $PSScriptRoot)) { throw '无法定位脚本目录' }

$data = Get-Content $Json -Raw | ConvertFrom-Json
$frameNames = @($data.frames.PSObject.Properties.Name)
$firstFrame = $data.frames.($frameNames[0]).frame
$fw = [int]$firstFrame.w
$fh = [int]$firstFrame.h
$sheetImg = [System.Drawing.Image]::FromFile((Resolve-Path $Sheet))

function Measure-FrameColor {
    param($Image, [int]$X, [int]$Y, [int]$W, [int]$H, [int]$AlphaMin)

    $rect = New-Object System.Drawing.Rectangle($X, $Y, $W, $H)
    $bmp = New-Object System.Drawing.Bitmap($W, $H)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.DrawImage($Image, (New-Object System.Drawing.Rectangle(0, 0, $W, $H)), $rect, [System.Drawing.GraphicsUnit]::Pixel)
    $g.Dispose()

    $locked = $bmp.LockBits((New-Object System.Drawing.Rectangle(0, 0, $W, $H)),
        [System.Drawing.Imaging.ImageLockMode]::ReadOnly,
        [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $stride = $locked.Stride
    $bytes = New-Object byte[] ($stride * $H)
    [System.Runtime.InteropServices.Marshal]::Copy($locked.Scan0, $bytes, 0, $bytes.Length)
    $bmp.UnlockBits($locked)
    $bmp.Dispose()

    $sr = 0.0; $sg = 0.0; $sb = 0.0; $sat = 0.0; $val = 0.0; $n = 0
    for ($y = 0; $y -lt $H; $y++) {
        $row = $y * $stride
        for ($x = 0; $x -lt $W; $x++) {
            $i = $row + $x * 4
            if ($bytes[$i + 3] -le $AlphaMin) { continue }
            $b = [int]$bytes[$i]; $gg = [int]$bytes[$i + 1]; $r = [int]$bytes[$i + 2]
            $sr += $r; $sg += $gg; $sb += $b; $n++
            $mx = [Math]::Max($r, [Math]::Max($gg, $b))
            $mn = [Math]::Min($r, [Math]::Min($gg, $b))
            $val += $mx
            if ($mx -gt 0) { $sat += ($mx - $mn) / $mx }
        }
    }
    if ($n -eq 0) { return $null }
    [pscustomobject]@{
        R = $sr / $n; G = $sg / $n; B = $sb / $n
        Sat = $sat / $n; Val = $val / $n; Px = $n
    }
}

# 逐 tag：对该 tag 所有帧求平均
$stats = @()
foreach ($tag in $data.meta.frameTags) {
    $r = 0.0; $g = 0.0; $b = 0.0; $s = 0.0; $v = 0.0; $c = 0
    for ($i = [int]$tag.from; $i -le [int]$tag.to; $i++) {
        $fr = $data.frames.($frameNames[$i])
        $m = Measure-FrameColor -Image $sheetImg -X ([int]$fr.frame.x) -Y ([int]$fr.frame.y) -W $fw -H $fh -AlphaMin $AlphaThreshold
        if ($null -eq $m) { continue }
        $r += $m.R; $g += $m.G; $b += $m.B; $s += $m.Sat; $v += $m.Val; $c++
    }
    if ($c -eq 0) { continue }
    $stats += [pscustomobject]@{
        Tag = $tag.name; Frames = $c
        R = $r / $c; G = $g / $c; B = $b / $c
        Sat = $s / $c; Val = $v / $c
        RB = ($r / $c) / [Math]::Max(1.0, ($b / $c))
    }
}
$sheetImg.Dispose()

$o = @()
$o += "精灵图色彩一致性报告"
$o += "  素材: $Sheet"
$o += "  尺寸: ${fw}x${fh} / $($frameNames.Count) 帧 / $($stats.Count) 个 tag"
$o += "  基准: $Baseline   实心像素判据: alpha > $AlphaThreshold"
$o += ""
$o += "R/G/B = 平均通道(0-255) | Sat = 平均饱和度(0-1) | Val = 平均明度(0-255) | R/B = 冷暖比(>1 偏暖)"
$o += ("{0,-14} {1,5} {2,8} {3,8} {4,8} {5,8} {6,8} {7,8}" -f 'tag', '帧数', 'R', 'G', 'B', 'Sat', 'Val', 'R/B')
foreach ($s in $stats) {
    $o += ("{0,-14} {1,5} {2,8:N1} {3,8:N1} {4,8:N1} {5,8:N3} {6,8:N1} {7,8:N3}" -f `
        $s.Tag, $s.Frames, $s.R, $s.G, $s.B, $s.Sat, $s.Val, $s.RB)
}

$base = $stats | Where-Object { $_.Tag -eq $Baseline }
if ($base) {
    $o += ""
    $o += "=== 相对 $Baseline 的偏移（正 = 更亮 / 更艳 / 更暖）==="
    $o += ("{0,-14} {1,9} {2,9} {3,9} {4,9} {5,8}" -f 'tag', 'Δ明度', 'Δ饱和', 'ΔR', 'ΔB', 'ΔR/B')
    foreach ($s in ($stats | Where-Object { $_.Tag -ne $Baseline } | Sort-Object { [Math]::Abs($_.Val - $base.Val) } -Descending)) {
        $o += ("{0,-14} {1,9:N1} {2,9:N3} {3,9:N1} {4,9:N1} {5,8:N3}" -f `
            $s.Tag, ($s.Val - $base.Val), ($s.Sat - $base.Sat), ($s.R - $base.R), ($s.B - $base.B), ($s.RB - $base.RB))
    }
} else {
    $o += ""
    $o += "⚠️ 找不到基准 tag '$Baseline'，跳过偏移计算（可用 -Baseline 指定其他 tag）"
}

$o += ""
$o += "=== 极值 ==="
$hiSat = $stats | Sort-Object Sat -Descending | Select-Object -First 1
$loSat = $stats | Sort-Object Sat | Select-Object -First 1
$hiVal = $stats | Sort-Object Val -Descending | Select-Object -First 1
$loVal = $stats | Sort-Object Val | Select-Object -First 1
$hiRB = $stats | Sort-Object RB -Descending | Select-Object -First 1
$loRB = $stats | Sort-Object RB | Select-Object -First 1
$o += ("  最艳 {0} ({1:N3})  ←→  最灰 {2} ({3:N3})    差 {4:N3}" -f $hiSat.Tag, $hiSat.Sat, $loSat.Tag, $loSat.Sat, ($hiSat.Sat - $loSat.Sat))
$o += ("  最亮 {0} ({1:N1})  ←→  最暗 {2} ({3:N1})    差 {4:N1}" -f $hiVal.Tag, $hiVal.Val, $loVal.Tag, $loVal.Val, ($hiVal.Val - $loVal.Val))
$o += ("  最暖 {0} ({1:N3})  ←→  最冷 {2} ({3:N3})    差 {4:N3}" -f $hiRB.Tag, $hiRB.RB, $loRB.Tag, $loRB.RB, ($hiRB.RB - $loRB.RB))
$o += ""
$o += "读法建议："
$o += "  · 明度差 > 5     → 切换时会明显闪一下，优先修"
$o += "  · 饱和度差 > 0.03 → 画风不一致，肉眼容易察觉"
$o += "  · 冷暖比差 > 0.05 → 一个偏暖一个偏冷，同屏对比时最刺眼"
$o += "  · 统计的是整帧均值，构图比例不同（如某动作露出更多浅色衣物）也会造成偏移；"
$o += "    请把结果当重点复查清单，最终以实机肉眼确认。"

$text = $o -join [Environment]::NewLine
if ($OutFile) {
    $text | Out-File -FilePath $OutFile -Encoding utf8
    Write-Output "报告已写入: $OutFile"
} else {
    Write-Output $text
}
