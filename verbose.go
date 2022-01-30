package main

import (
	"fmt"
	"os"
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
	message := toMessage("[TRACE]", fmt_, args...)
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
	message := toMessage("[INFO ]", fmt_, args...)
	if v.VerboseEnabled {
		fmt.Printf(message)
	}
	if v.LogEnabled && len(v.LogFile) > 0 {
		os.WriteFile(v.LogFile, []byte(message), os.ModeAppend)
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
	// TODO: Truncat Log
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
