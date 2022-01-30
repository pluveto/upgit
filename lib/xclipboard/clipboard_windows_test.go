/**
 * Modified based on
 * https://github.com/robinchenyu/clipboard_go
 */

package xclipboard

import (
	"io/fs"
	"os"
	"strings"
	"testing"
)

func TestReadClipboard(t *testing.T) {
	buff, err := ReadClipboardImage()
	if err != nil {
		if strings.Contains(err.Error(), "IsClipboardFormatAvailable") {
			t.Skipf("Skipped because clipboard has no image")
			return
		}
		t.Errorf(err.Error())
	}
	os.WriteFile("cliboard_test.png", buff, os.FileMode(fs.ModePerm))
}
