package main

import (
	"errors"
	"fmt"
	"os"
	"path"

	"github.com/alexflint/go-arg"
	"github.com/pluveto/upgit/lib/xgithub"
	"github.com/pluveto/upgit/lib/xlog"
)

type ExtListCmd struct {
}

type ExtAddCmd struct {
	Name string `arg:"positional"`
}

type ExtRemoveCmd struct {
	Name string `arg:"positional"`
}

type ExtCmd struct {
	List   *ExtListCmd   `arg:"subcommand:list,subcommand:ls"`
	Add    *ExtAddCmd    `arg:"subcommand:add,subcommand:install"`
	Remove *ExtRemoveCmd `arg:"subcommand:remove,subcommand:rm"`
}

type ExtArgs struct {
	Ext *ExtCmd `arg:"subcommand:ext"`
}

var extArgs ExtArgs

func extSubcommand() {
	err := arg.Parse(&extArgs)
	if err != nil || extArgs.Ext == nil {
		os.Stderr.WriteString("Error: " + err.Error() + "\n")
		printExtHelp()
		return
	}

	switch {
	case extArgs.Ext.List != nil:
		ls, err := xgithub.ListFolder("pluveto/upgit", "/extensions")
		if err != nil {
			xlog.AbortErr(err)
		}
		fmt.Println("Extensions (install with FULL name):")
		for i, v := range ls {
			fmt.Printf("%d. %s\n", i, v.Name)
		}
		os.Exit(0)

	case extArgs.Ext.Add != nil:
		extName := extArgs.Ext.Add.Name
		if len(extName) == 0 {
			xlog.AbortErr(errors.New("extension name is required"))
		}
		buf, err := xgithub.GetFile("pluveto/upgit", "master", "/extensions/"+extName)
		if err != nil {
			xlog.AbortErr(errors.New("extension not found or network error: " + err.Error()))
		}
		// save buf
		file, err := os.Create(path.Join(MustGetApplicationPath("extensions"), extName))
		defer file.Close()
		if err != nil {
			xlog.AbortErr(err)
		}
		_, err = file.Write(buf)
		if err != nil {
			xlog.AbortErr(err)
		}
		fmt.Println("Extension installed:", extName)
		os.Exit(0)

	case extArgs.Ext.Remove != nil:
		extName := extArgs.Ext.Remove.Name
		if len(extName) == 0 {
			xlog.AbortErr(errors.New("extension name is required"))
		}
		err := os.Remove(path.Join(MustGetApplicationPath("extensions"), extName))
		if err != nil {
			xlog.AbortErr(err)
		}
		fmt.Println("Extension removed:", extName)
		os.Exit(0)
	}
	os.Stderr.WriteString("Unknown subcommand\n")
	printExtHelp()
	os.Exit(0)
}

func printExtHelp() {
	os.Stdout.WriteString("Usage: upgit ext [list|add|remove]\n")
}
