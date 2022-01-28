package main

import "os"

type Result[T any] struct {
	value T
	err   error
}

func (r Result[T]) Ok() bool {
	return r.err == nil
}

func (r Result[T]) ValueOrDefault(default_ T) T {
	if r.err == nil {
		return r.value
	}
	return default_
}

func (r Result[T]) ValueOrPanic() T {
	if r.err == nil {
		return r.value
	}
	panic(r.err)
}

func (r Result[T]) ValueOrExit() T {
	if r.err == nil {
		return r.value
	}
	abortErr(r.err)
	panic(r.err)
}

func abortErr(err error) {
	if err != nil {
		os.Stderr.WriteString(err.Error())
		os.Exit(1)
	}
}

func panicErr(err error) {
	if err != nil {
		panic(err)
	}
}
