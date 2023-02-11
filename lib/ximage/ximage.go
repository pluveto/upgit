package ximage

import (
	"bytes"
	"image"
	_ "image/gif"
	_ "image/jpeg"
	"image/png"

	"github.com/nfnt/resize"
)

func Scale(src []byte, width, height uint) ([]byte, error) {
	reader := bytes.NewReader(src)
	img, _, err := image.Decode(reader)
	if err != nil {
		return nil, err
	}
	newImage := resize.Resize(width, height, img, resize.Lanczos3)
	buf := new(bytes.Buffer)
	err = png.Encode(buf, newImage)
	if err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

func GetSize(src []byte) (uint, uint, error) {
	reader := bytes.NewReader(src)
	image, _, err := image.Decode(reader)
	if err != nil {
		return 0, 0, err
	}
	return uint(image.Bounds().Dx()), uint(image.Bounds().Dy()), nil
}
