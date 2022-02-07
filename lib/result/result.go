package result

type Result[T any] struct {
	Value T
	Err   error
}

func (r Result[T]) Ok() bool {
	return r.Err == nil
}

func FromGoRet[T any](in ...interface{}) Result[T] {
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

type ErrorHandler func(err error)

var AbortErr ErrorHandler

func (r Result[T]) ValueOrExit() T {
	if r.Err == nil {
		return r.Value
	}
	if nil != AbortErr {
		AbortErr(r.Err)
	}
	panic(r.Err)
}
