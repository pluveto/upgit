package result

type Result[T any] struct {
	Value T
	Err   error
}

func (r Result[T]) Ok() bool {
	return r.Err == nil
}

// From 
// In Golang we usually return by the format of `data ,err`. 
// This function convert it to a Result[T], so that you can handle error in a tidy way
func From[T any](in ...interface{}) Result[T] {
	if len(in) != 2 {
		panic("unexpected number of return values")
	}
	if nil != in[1] {
		return Result[T]{
			Err: in[1].(error),
		}
	}
	return Result[T]{
		Value: in[0].(T),
	}
}

func (r Result[T]) ValueOrDefault(default_ T) T {
	if r.Err == nil {
		return r.Value
	}
	return default_
}

func (r Result[T]) ValueOrPanic() T {
	if r.Err == nil {
		return r.Value
	}
	panic(r.Err)
}

type AbortHandler func(err error)

var AbortErr AbortHandler

func (r Result[T]) ValueOrExit() T {
	if r.Err == nil {
		return r.Value
	}
	if nil != AbortErr {
		AbortErr(r.Err)
	}
	panic(r.Err)
}
