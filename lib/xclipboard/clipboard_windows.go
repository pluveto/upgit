/**
 * Modified based on
 * https://github.com/robinchenyu/clipboard_go
 */
package xclipboard

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"image/png"
	"syscall"
	"unsafe"

	"golang.org/x/image/bmp"
)

// see https://docs.microsoft.com/en-us/windows/win32/dataxchg/standard-clipboard-formats
const (
	CF_BITMAP      = 2
	CF_DIB         = 8
	CF_UNICODETEXT = 13
	CF_DIBV5       = 17
)

// see https://docs.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapfileheader
type fileHeader struct {
	bfType      uint16
	bfSize      uint32
	bfReserved1 uint16
	bfReserved2 uint16
	bfOffBits   uint32
}

type infoHeader struct {
	iSize          uint32
	iWidth         uint32
	iHeight        uint32
	iPLanes        uint16
	iBitCount      uint16
	iCompression   uint32
	iSizeImage     uint32
	iXPelsPerMeter uint32
	iYPelsPerMeter uint32
	iClrUsed       uint32
	iClrImportant  uint32
}

var (
	user32                     = syscall.MustLoadDLL("user32")
	openClipboard              = user32.MustFindProc("OpenClipboard")
	closeClipboard             = user32.MustFindProc("CloseClipboard")
	getClipboardData           = user32.MustFindProc("GetClipboardData")
	isClipboardFormatAvailable = user32.MustFindProc("IsClipboardFormatAvailable")

	kernel32     = syscall.NewLazyDLL("kernel32")
	globalLock   = kernel32.NewProc("GlobalLock")
	globalUnlock = kernel32.NewProc("GlobalUnlock")
)

func copyInfoHeader(dst *byte, pSrc *infoHeader) {
	pdst := (*infoHeader)(unsafe.Pointer(dst))
	pdst.iSize = pSrc.iSize
	pdst.iWidth = pSrc.iWidth
	pdst.iHeight = pSrc.iHeight
	pdst.iPLanes = pSrc.iPLanes
	pdst.iBitCount = pSrc.iBitCount
	pdst.iCompression = pSrc.iCompression
	pdst.iSizeImage = pSrc.iSizeImage
	pdst.iXPelsPerMeter = pSrc.iXPelsPerMeter
	pdst.iYPelsPerMeter = pSrc.iYPelsPerMeter
	pdst.iClrUsed = pSrc.iClrUsed
	pdst.iClrImportant = pSrc.iClrImportant
}

func readUint16(b []byte) uint16 {
	return uint16(b[0]) | uint16(b[1])<<8
}

func readUint32(b []byte) uint32 {
	return uint32(b[0]) | uint32(b[1])<<8 | uint32(b[2])<<16 | uint32(b[3])<<24
}

func ReadClipboardImage() (buf []byte, err error) {
	const (
		fileHeaderLen = 14
		infoHeaderLen = 40
	)

	succ, _, err := openClipboard.Call(0)
	if succ == 0 {
		return nil, fmt.Errorf("failed to open clipboard: " + err.Error())
	}
	defer closeClipboard.Call()

	succ, _, err = isClipboardFormatAvailable.Call(CF_DIB)
	if succ == 0 {
		return nil, fmt.Errorf("false on IsClipboardFormatAvailable: " + err.Error())
	}

	hClipObj, _, err := getClipboardData.Call(CF_DIB)
	if succ == 0 {
		err = syscall.GetLastError()
		return nil, fmt.Errorf("failed to get clipboard data: " + err.Error())
	}

	pMemBlk, _, err := globalLock.Call(hClipObj)
	if pMemBlk == 0 {
		return nil, fmt.Errorf("failed to call global lock: " + err.Error())
	}
	defer globalUnlock.Call(hClipObj)

	clipObjHeader := (*infoHeader)(unsafe.Pointer(pMemBlk))
	dataSize := clipObjHeader.iSizeImage + fileHeaderLen + infoHeaderLen

	if clipObjHeader.iSizeImage == 0 && clipObjHeader.iCompression == 0 {
		iSizeImage := clipObjHeader.iHeight * ((clipObjHeader.iWidth*uint32(clipObjHeader.iBitCount)/8 + 3) &^ 3)
		dataSize += iSizeImage
	}
	bmpBuf := new(bytes.Buffer)
	binary.Write(bmpBuf, binary.LittleEndian, uint16('B')|(uint16('M')<<8))
	binary.Write(bmpBuf, binary.LittleEndian, uint32(dataSize))
	binary.Write(bmpBuf, binary.LittleEndian, uint32(0))
	const sizeof_colorbar = 0
	binary.Write(bmpBuf, binary.LittleEndian, uint32(fileHeaderLen+infoHeaderLen+sizeof_colorbar))
	j := 0
	for i := fileHeaderLen; i < int(dataSize); i++ {
		binary.Write(bmpBuf, binary.BigEndian, *(*byte)(unsafe.Pointer(pMemBlk + uintptr(j))))
		j++
	}
	return bmpToPng(bmpBuf)
}

func bmpToPng(bmpBuf *bytes.Buffer) (buf []byte, err error) {
	var f bytes.Buffer
	original_image, err := bmp.Decode(bmpBuf)
	if err != nil {
		return nil, err
	}
	err = png.Encode(&f, original_image)
	if err != nil {
		return nil, err
	}
	return f.Bytes(), nil
}
