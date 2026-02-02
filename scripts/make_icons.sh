#!/usr/bin/env bash
makeicns() {
  local png="$1"
  local base="${png%.*}"
  mkdir -p "$base.iconset"
  echo "$base.iconset"

  sips -z 16 16   "$png" --out "$base.iconset/icon_16x16.png"
  sips -z 32 32   "$png" --out "$base.iconset/icon_16x16@2x.png"

  sips -z 32 32   "$png" --out "$base.iconset/icon_32x32.png"
  sips -z 64 64   "$png" --out "$base.iconset/icon_32x32@2x.png"

  sips -z 64 64   "$png" --out "$base.iconset/icon_64x64.png"
  sips -z 128 128   "$png" --out "$base.iconset/icon_64x64@2x.png"

  sips -z 128 128 "$png" --out "$base.iconset/icon_128x128.png"
  sips -z 256 256 "$png" --out "$base.iconset/icon_128x128@2x.png"

  sips -z 256 256 "$png" --out "$base.iconset/icon_256x256.png"
  sips -z 512 512 "$png" --out "$base.iconset/icon_256x256@2x.png"

  sips -z 512 512 "$png" --out "$base.iconset/icon_512x512.png"
  cp "$png"              "$base.iconset/icon_512x512@2x.png"

  iconutil -c icns "$base.iconset"
}
makeicns $1
