package restate

type record struct{ userName string }

func f(r *record, userName string) {
	// the login, never the display form
	r.userName = userName
}
