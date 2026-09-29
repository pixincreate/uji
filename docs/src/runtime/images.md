# Images

## uji.image.fit(data, edge, bytes)

Checks that `data` holds a PNG, JPEG, GIF or WebP image and returns it ready to
send. A photo with an EXIF orientation is turned upright, and an image whose
long side passes `edge` pixels is scaled down. When the result is still larger
than `bytes`, it is saved as JPEG at lower quality, and then at half the size
until it fits.

The result is the image bytes, the media type such as `"image/png"`, the width
and the height. An image that needs no change comes back as it was. Anything
else gives `nil` and an error message.

```lua
local data, media_type, width, height = uji.image.fit(uji.fs.read("shot.png"), 1568, 3 * 1024 * 1024)
```
