package fml

processors: [string]: #Processor

#Processor: #DefinitionBase & {
	spec: #ProcessorSpec

	#ProcessorSpec: {
		name: string
	}
}

// Ref

#RefProcessor: or([for _, processor in processors {processor.id}])

// Definitions

processors: {
	[Name=string]: id: Name

	exr_processor_pro: #Processor & {
		spec: name: "EXR Processor Pro"
	}

	exr_processor_ii: #Processor & {
		spec: name: "EXR Processor II"
	}

	x_processor_pro: #Processor & {
		spec: name: "X-Processor Pro"
	}

	x_processor_4: #Processor & {
		spec: name: "X-Processor 4"
	}

	x_processor_5: #Processor & {
		spec: name: "X-Processor 5"
	}
}
