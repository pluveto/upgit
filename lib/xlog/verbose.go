package xlog

import (
	"fmt"
	"os"
	"runtime/debug"
	"strings"
	"time"

	"github.com/pelletier/go-toml/v2"
	"github.com/pluveto/upgit/lib/xio"
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
	_, message := toMessage("[TRACE] ", fmt_, args...)
	if v.VerboseEnabled {
		fmt.Printf(message)
	}
}

func toMessage(level, fmt_ string, args ...interface{}) (string, string) {
	// better format multiple lines output
	fmtMulLine_ := strings.TrimRight(strings.ReplaceAll(fmt_, "\n", "\n        "), " \n")
	messageNoTime := fmt.Sprintf(level+fmtMulLine_+"\n", args...)
	message := time.Now().String() + messageNoTime
	return message, messageNoTime
}

func (v Verbose) Info(fmt_ string, args ...interface{}) {
	v.Log("[INFO ] ", fmt_, args...)
}

func (v Verbose) Error(fmt_ string, args ...interface{}) {
	v.Log("[ERROR] ", fmt_, args...)
}

func (v Verbose) Log(level, fmt_ string, args ...interface{}) {
	log, message := toMessage(level, fmt_, args...)
	if v.VerboseEnabled {
		fmt.Printf(message)
	}
	if v.LogEnabled && len(v.LogFile) > 0 {
		xio.AppendToFile(v.LogFile, []byte(log))
		if strings.Contains(level, "[ERROR]") {
			xio.AppendToFile(v.LogFile, []byte(debug.Stack()))
		}
	}
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

func AbortErr(err error) {
	if err != nil {
		GVerbose.Error("abort: " + err.Error())
		os.Stderr.WriteString(err.Error())
		os.Exit(1)
	}
}

func panicErr(err error) {
	if err != nil {
		GVerbose.Error("panic: " + err.Error())
		panic(err)
	}
}

func panicErrWithoutLog(err error) {
	if err != nil {
		panic(err)
	}
}
