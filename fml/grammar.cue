package fml

import "list"

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

	#Predicate: #Predicates.#Logic | #Predicates.#Leaf | bool

	#Predicates: {
		#Logic:
			#Predicates.#PredicateAll |
			#Predicates.#PredicateAny |
			#Predicates.#PredicateNot

		#PredicateAll: {all: [...#Predicate]}

		#PredicateAny: {any: [...#Predicate]}

		#PredicateNot: {not: #Predicate}

		_LeafScope: {
			scope: #Scope | *"current"
			if !_scoped {
				scope: "current"
			}
		}

		#Leaf: _LeafScope & {
			ref: or(_ids.all)

			if list.Contains(_ids.i, ref) {
				{equals: int} |
				{in: [...int]} |
				{lt: int} |
				{lte: int} |
				{gt: int} |
				{gte: int} |
				{min: int, max: int} |
				{present: bool}
			}

			if list.Contains(_ids.f, ref) {
				{equals: float} |
				{in: [...float]} |
				{lt: float} |
				{lte: float} |
				{gt: float} |
				{gte: float} |
				{min: float, max: float} |
				{present: bool}
			}

			if list.Contains(_ids.s, ref) {
				{equals: string} |
				{in: [...string]} |
				{present: bool}
			}

			if list.Contains(_ids.e, ref) {
				{equals: or([for v in options[ref].spec.rules.variants {v.id}])} |
				{in: [...or([for v in options[ref].spec.rules.variants {v.id}])]} |
				{present: bool}
			}
		}
	}
}

#Severity: "error" | "warning" | "info"

#RuleBase: {
	severity: #Severity | *"error"
	message:  string
	when:     _
}
