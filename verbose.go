package main

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime/debug"
	"strings"
	"time"

	"github.com/pelletier/go-toml/v2"
)

// GVerbose is a global verbose
var GVerbose Verbose

type Verbose struct {
	VerboseEnabled bool
	LogEnabled     bool
	LogFile        string
	LogFileMaxSize int64
}

func (v Verbose) Trace(fmt_ string, args ...interface{}) {
	message := toMessage("[TRACE] ", fmt_, args...)
	if v.VerboseEnabled {
		fmt.Printf(message)
	}
}

func toMessage(level, fmt_ string, args ...interface{}) string {
	// better format multiple lines output
	fmtMulLine_ := strings.TrimRight(strings.ReplaceAll(fmt_, "\n", "\n        "), " \n")
	message := fmt.Sprintf(time.Now().String()+level+fmtMulLine_+"\n", args...)
	return message
}

func (v Verbose) Info(fmt_ string, args ...interface{}) {
	v.Log("[INFO ] ", fmt_, args...)
}

func (v Verbose) Error(fmt_ string, args ...interface{}) {
	v.Log("[ERROR] ", fmt_, args...)
}

func (v Verbose) Log(level, fmt_ string, args ...interface{}) {
	message := toMessage(level, fmt_, args...)
	if v.VerboseEnabled {
		fmt.Printf(message)
	}
	if v.LogEnabled && len(v.LogFile) > 0 {
		appendToFile(v.LogFile, []byte(message))
		if strings.Contains(level, "[ERROR]") {
			appendToFile(v.LogFile, []byte(debug.Stack()))
		}
	}
}

func appendToFile(filePath string, data []byte) {
	err := os.MkdirAll(filepath.Dir(filePath), 0755)
	panicErrWithoutLog(err)
	file, err := os.OpenFile(filePath, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0755)
	defer file.Close()
	panicErrWithoutLog(err)
	file.Write(data)
}

func (v Verbose) TruncatLog() {
	doTrunc := false
	info, err := os.Stat(v.LogFile)
	if err == nil && v.LogFileMaxSize != 0 {
		if info.Size() >= v.LogFileMaxSize {
			doTrunc = true
		}
	}
	if !doTrunc {
		return
	}
	var truncSize = v.LogFileMaxSize / 2
	file, err := os.OpenFile(v.LogFile, os.O_RDWR, 0755)
	panicErrWithoutLog(err)
	defer file.Close()
	file.Seek(truncSize, 0)
	file.Truncate(truncSize)

}

func (v Verbose) TraceStruct(s interface{}) {
	if !v.VerboseEnabled {
		return
	}
	b, err := toml.Marshal(s)
	if err == nil {
		GVerbose.Trace(string(b))
	} else {
		GVerbose.Trace(err.Error())
	}
}
