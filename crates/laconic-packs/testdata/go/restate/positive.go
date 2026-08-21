package restate

type record struct{ userName string }

func f(r *record, userName string) {
	// the user name
	r.userName = userName
}
