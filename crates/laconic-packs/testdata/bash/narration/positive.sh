f() {
  # changed to use an array for lookups
  m=()
  echo "${m[@]}"

  # Previously this returned an error.
  n=1
  echo "$n"
}
