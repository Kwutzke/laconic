f() {
  # TODO(#456) fix the parser
  x=1
  echo "$x"

  # FIXME(KAT-12) leaks a file descriptor on the error path
  y=2
  echo "$y"
}
