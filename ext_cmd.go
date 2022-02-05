package main

import (
	"errors"
	"fmt"
	"os"
	"path"

	"github.com/alexflint/go-arg"
	"github.com/pluveto/upgit/lib/xgithub"
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
	List   *ExtListCmd   `arg:"subcommand:list"`
	Add    *ExtAddCmd    `arg:"subcommand:add"`
	Remove *ExtRemoveCmd `arg:"subcommand:remove"`
}

type Ext struct {
	Ext *ExtCmd `arg:"subcommand:ext"`
}

var args Ext

func TryExtCmd() {
	err := arg.Parse(&args)
	if err != nil || args.Ext == nil {
		return
	}

	switch {
	case args.Ext.List != nil:
		ls, err := xgithub.ListFolder("pluveto/upgit", "/extensions")
		if err != nil {
			abortErr(err)
		}
		fmt.Println("Extensions (install with FULL name):")
		for i, v := range ls {
			fmt.Printf("%d. %s\n", i, v.Name)
		}
		os.Exit(0)

	case args.Ext.Add != nil:
		extName := args.Ext.Add.Name
		if len(extName) == 0 {
			abortErr(errors.New("extension name is required"))
		}
		buf, err := xgithub.GetFile("pluveto/upgit", "master", "/extensions/"+extName)
		if err != nil {
			abortErr(errors.New("extension not found or network error: " + err.Error()))
		}
		// save buf
		file, err := os.Create(path.Join(MustGetApplicationPath("extensions"), extName))
		defer file.Close()
		if err != nil {
			abortErr(err)
		}
		_, err = file.Write(buf)
		if err != nil {
			abortErr(err)
		}
		fmt.Println("Extension installed:", extName)
		os.Exit(0)

	case args.Ext.Remove != nil:
		extName := args.Ext.Remove.Name
		if len(extName) == 0 {
			abortErr(errors.New("extension name is required"))
		}
		err := os.Remove(path.Join(MustGetApplicationPath("extensions"), extName))
		if err != nil {
			abortErr(err)
		}
		fmt.Println("Extension removed:", extName)
		os.Exit(0)
	}
	os.Stderr.WriteString("Unknown subcommand\n")
	p, _ := arg.NewParser(arg.Config{}, &args)
	p.WriteUsage(os.Stderr)
	os.Exit(0)
}
