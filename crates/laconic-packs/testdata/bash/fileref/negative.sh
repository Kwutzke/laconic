f() {
  # runs after scripts/lint-go.sh, which leaves the tree formatted
  x=1
  echo "$x"

  # config.json holds the defaults
  y=2
  echo "$y"
}
