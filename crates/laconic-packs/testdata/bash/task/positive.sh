f() {
  # TODO fix the parser
  x=1
  echo "$x"

  # FIXME leaks a file descriptor on the error path
  y=2
  echo "$y"
}
