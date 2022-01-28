package main

import (
	"fmt"
	"strings"

	"github.com/pelletier/go-toml/v2"
)

// GVerbose is a global verbose
var GVerbose Verbose

type Verbose struct {
	Enabled bool
}


func (v Verbose) Trace(fmt_ string, args ...interface{}) {
	if !v.Enabled {
		return
	}
	// better format multiple lines output
	fmtMulLine_ := strings.TrimRight(strings.ReplaceAll(fmt_, "\n", "\n        "), " \n")
	fmt.Printf("[TRACE] "+fmtMulLine_+"\n", args...)
}

func (v Verbose) TraceStruct(s interface{}) {
	if !v.Enabled {
		return
	}
	b, err := toml.Marshal(s)
	if err == nil {
		GVerbose.Trace(string(b))
	} else {
		GVerbose.Trace(err.Error())
	}

}
