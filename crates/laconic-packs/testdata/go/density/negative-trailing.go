package density

func trailing() int {
	limit := 32 // the SAP batch endpoint rejects more than 32 ids in one call
	total := 0
	for i := 0; i < limit; i++ {
		total += i
	}
	return total
}
