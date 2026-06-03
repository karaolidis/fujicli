package fml

#Scope: "current" | "original"

#GrammarBase: {
	_ids: {
		all: [...string]
		i: [...string]
		f: [...string]
		s: [...string]
		e: [...string]
	}

	_scoped: bool | *false

	#Predicate: #Predicates.#Logic | #Predicates.#Leaf

	#Predicates: {
		#Logic:
			#Predicates.#PredicateAll |
			#Predicates.#PredicateAny |
			#Predicates.#PredicateNot

		#PredicateAll: {all: [...#Predicate]}

		#PredicateAny: {any: [...#Predicate]}

		#PredicateNot: {not: #Predicate}

		#Leaf:
			#Predicates.#PredicateInteger |
			#Predicates.#PredicateFloat |
			#Predicates.#PredicateString |
			#Predicates.#PredicateEnum |
			#Predicates.#PredicatePresent |
			#Predicates.#PredicateBool

		_LeafScope: {
			scope: #Scope | *"current"
			if !_scoped {
				scope: "current"
			}
		}

		#PredicateInteger:
			#PredicateIntegerEquals |
			#PredicateIntegerIn |
			#PredicateIntegerLessThan |
			#PredicateIntegerLessThanOrEqual |
			#PredicateIntegerGreaterThan |
			#PredicateIntegerGreaterThanOrEqual |
			#PredicateIntegerBetween

		#PredicateIntegerEquals: _LeafScope & {ref: or(_ids.i), equals: int}
		#PredicateIntegerIn: _LeafScope & {ref: or(_ids.i), in: [...int]}
		#PredicateIntegerLessThan: _LeafScope & {ref: or(_ids.i), lt: int}
		#PredicateIntegerLessThanOrEqual: _LeafScope & {ref: or(_ids.i), lte: int}
		#PredicateIntegerGreaterThan: _LeafScope & {ref: or(_ids.i), gt: int}
		#PredicateIntegerGreaterThanOrEqual: _LeafScope & {ref: or(_ids.i), gte: int}
		#PredicateIntegerBetween: _LeafScope & {ref: or(_ids.i), min: int, max: int}

		#PredicateFloat:
			#PredicateFloatEquals |
			#PredicateFloatIn |
			#PredicateFloatLessThan |
			#PredicateFloatLessThanOrEqual |
			#PredicateFloatGreaterThan |
			#PredicateFloatGreaterThanOrEqual |
			#PredicateFloatBetween

		#PredicateFloatEquals: _LeafScope & {ref: or(_ids.f), equals: float}
		#PredicateFloatIn: _LeafScope & {ref: or(_ids.f), in: [...float]}
		#PredicateFloatLessThan: _LeafScope & {ref: or(_ids.f), lt: float}
		#PredicateFloatLessThanOrEqual: _LeafScope & {ref: or(_ids.f), lte: float}
		#PredicateFloatGreaterThan: _LeafScope & {ref: or(_ids.f), gt: float}
		#PredicateFloatGreaterThanOrEqual: _LeafScope & {ref: or(_ids.f), gte: float}
		#PredicateFloatBetween: _LeafScope & {ref: or(_ids.f), min: float, max: float}

		#PredicateString:
			#PredicateStringEquals |
			#PredicateStringIn

		#PredicateStringEquals: _LeafScope & {ref: or(_ids.s), equals: string}
		#PredicateStringIn: _LeafScope & {ref: or(_ids.s), in: [...string]}

		#PredicateEnum:
			#PredicateEnumEquals |
			#PredicateEnumIn

		#PredicateEnumEquals: _LeafScope & {
			ref: or(_ids.e)
			equals: or([for v in options[ref].spec.rules.variants {v.id}])
		}
		#PredicateEnumIn: _LeafScope & {ref: or(_ids.e), in: [...or([for v in options[ref].spec.rules.variants {v.id}])]}

		#PredicatePresent: _LeafScope & {ref: or(_ids.all), present: bool}

		#PredicateBool: bool
	}

	#Assignment:
		#Assignments.#AssignmentInteger |
		#Assignments.#AssignmentFloat |
		#Assignments.#AssignmentString |
		#Assignments.#AssignmentEnum |
		#Assignments.#AssignmentClear

	#Assignments: {
		#AssignmentInteger: {ref: or(_ids.i), value: int}
		#AssignmentFloat: {ref: or(_ids.f), value: float}
		#AssignmentString: {ref: or(_ids.s), value: string}
		#AssignmentEnum: {ref: or(_ids.e), value: or([for v in options[ref].spec.rules.variants {v.id}])}
		#AssignmentClear: {ref: or(_ids.all), present: false}
	}
}

#Severity: "error" | "warning" | "info"

#RuleBase: {
	severity: #Severity | *"error"
	message:  string
	when:     _
}

#TransformationBase: {
	when?: _
	apply: _
}
