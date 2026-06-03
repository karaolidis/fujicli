package fml

import "list"

cameras: [string]: #Camera

#Camera: #DefinitionBase & {
	spec: #Spec

	#Spec: {
		name:       string
		generation: #RefGeneration

		_generation: generations["\(generation)"].spec

		usb: #USB

		features?: #Features

		#USB: {
			#FujifilmVendorID:          0x04cb
			#DefaultCameraUSBChunkSize: 1024 * 1024

			vendor_id:  uint16 | *#FujifilmVendorID
			product_id: uint16
			chunk_size: uint | *#DefaultCameraUSBChunkSize
		}

		#Features: {
			backup?:     true
			simulation?: #Simulation
			render?:     #Render

			#Simulation: {
				slots: uint
				settings: [...#Setting]

				#Setting: {
					id:  string | *ref
					ref: #RefOption
				}

				#Grammar: #GrammarBase & {
					_ids: {
						all: [for _, setting in settings {setting.id}]
						i: [for _, setting in settings if options[setting.ref].spec.kind == "integer" {setting.id}]
						f: [for _, setting in settings if options[setting.ref].spec.kind == "float" {setting.id}]
						s: [for _, setting in settings if options[setting.ref].spec.kind == "string" {setting.id}]
						e: [for _, setting in settings if options[setting.ref].spec.kind == "enum" {setting.id}]
					}
				}

				transformations?: [...#Transformation]

				#Transformation: #TransformationBase & {
					when?: #Grammar.#Predicate
					apply: [...#Grammar.#Assignment]
				}

				rules?: [...#Rule]

				#Rule: #RuleBase & {
					when: #Grammar.#Predicate
				}

				_validation: {
					ids: list.UniqueItems & [for s in settings {s.id}]
				}
			}

			#Render: {
				profile_code: uint32

				fields: [...#Field]

				#Field: {
					id:          string | *ref
					skip_read?:  true
					skip_write?: true
					ref?:        #RefOption
				}

				#Grammar: #GrammarBase & {
					_scoped: true
					_ids: {
						all: [for _, field in fields {field.id}]
						i: list.Concat([
							[for _, field in fields if field.ref == _|_ {field.id}],
							[for _, field in fields if field.ref != _|_ if options[field.ref].spec.kind == "integer" {field.id}],
						])
						f: [for _, field in fields if field.ref != _|_ if options[field.ref].spec.kind == "float" {field.id}]
						s: [for _, field in fields if field.ref != _|_ if options[field.ref].spec.kind == "string" {field.id}]
						e: [for _, field in fields if field.ref != _|_ if options[field.ref].spec.kind == "enum" {field.id}]
					}
				}

				#TransformationGrammar: #GrammarBase & {
					_scoped: false
					_ids:    #Grammar._ids
				}

				transformations?: [...#Transformation]

				#Transformation: #TransformationBase & {
					when?: #TransformationGrammar.#Predicate
					apply: [...#TransformationGrammar.#Assignment]
				}

				rules?: [...#Rule]

				#Rule: #RuleBase & {
					when: #Grammar.#Predicate
				}

				_validation: {
					ids: list.UniqueItems & [for f in fields {f.id}]
				}
			}
		}
	}
}

// Ref

#RefCamera: or([for _, camera in cameras {camera.id}])

// Definitions

cameras: {
	[Name=string]: id: Name

	x_e1: #Camera & {
		spec: {
			name:       "FUJIFILM X-E1"
			generation: "x_trans"
			usb: product_id: 0x0283
		}
	}

	x_m1: #Camera & {
		spec: {
			name:       "FUJIFILM X-M1"
			generation: "x_trans"
			usb: product_id: 0x02b6
		}
	}

	x70: #Camera & {
		spec: {
			name:       "FUJIFILM X70"
			generation: "x_trans_ii"
			usb: product_id: 0x02ba
		}
	}

	x_e2: #Camera & {
		spec: {
			name:       "FUJIFILM X-E2"
			generation: "x_trans_ii"
			usb: product_id: 0x02b5
		}
	}

	x_t1: #Camera & {
		spec: {
			name:       "FUJIFILM X-T1"
			generation: "x_trans_ii"
			usb: product_id: 0x02bf
		}
	}

	x_t10: #Camera & {
		spec: {
			name:       "FUJIFILM X-T10"
			generation: "x_trans_ii"
			usb: product_id: 0x02c8
		}
	}

	x100f: #Camera & {
		spec: {
			name:       "FUJIFILM X100F"
			generation: "x_trans_iii"
			usb: product_id: 0x02d1
		}
	}

	x_e3: #Camera & {
		spec: {
			name:       "FUJIFILM X-E3"
			generation: "x_trans_iii"
			usb: product_id: 0x02d6
		}
	}

	x_h1: #Camera & {
		spec: {
			name:       "FUJIFILM X-H1"
			generation: "x_trans_iii"
			usb: product_id: 0x02d7
		}
	}

	x_pro2: #Camera & {
		spec: {
			name:       "FUJIFILM X-Pro2"
			generation: "x_trans_iii"
			usb: product_id: 0x02cb
		}
	}

	x_t2: #Camera & {
		spec: {
			name:       "FUJIFILM X-T2"
			generation: "x_trans_iii"
			usb: product_id: 0x02cd
		}
	}

	x_t20: #Camera & {
		spec: {
			name:       "FUJIFILM X-T20"
			generation: "x_trans_iii"
			usb: product_id: 0x02d4
		}
	}

	x100v: #Camera & {
		spec: {
			name:       "FUJIFILM X100V"
			generation: "x_trans_iv"
			usb: product_id: 0x02e5
		}
	}

	x_e4: #Camera & {
		spec: {
			name:       "FUJIFILM X-E4"
			generation: "x_trans_iv"
			usb: product_id: 0x02e8
		}
	}

	x_pro3: #Camera & {
		spec: {
			name:       "FUJIFILM X-Pro3"
			generation: "x_trans_iv"
			usb: product_id: 0x02e4
		}
	}

	x_s10: #Camera & {
		spec: {
			name:       "FUJIFILM X-S10"
			generation: "x_trans_iv"
			usb: product_id: 0x02ea
		}
	}

	x_t3: #Camera & {
		spec: {
			name:       "FUJIFILM X-T3"
			generation: "x_trans_iv"
			usb: product_id: 0x02dd
		}
	}

	x_t4: #Camera & {
		spec: {
			name:       "FUJIFILM X-T4"
			generation: "x_trans_iv"
			usb: product_id: 0x02e6
		}
	}

	x100vi: #Camera & {
		spec: {
			name:       "FUJIFILM X100VI"
			generation: "x_trans_v"
			usb: product_id: 0x0305
		}
	}

	x_h2: #Camera & {
		spec: {
			name:       "FUJIFILM X-H2"
			generation: "x_trans_v"
			usb: product_id: 0x02f2
		}
	}

	x_h2s: #Camera & {
		spec: {
			name:       "FUJIFILM X-H2S"
			generation: "x_trans_v"
			usb: product_id: 0x02f0
		}
	}

	x_s20: #Camera & {
		spec: {
			name:        "FUJIFILM X-S20"
			generation:  "x_trans_iv"
			_generation: _

			usb: product_id: 0x02f7

			features: {
				backup: true

				simulation: {
					slots:    4
					settings: _generation._simulation.settings
					rules:    _generation._simulation.rules
				}
			}
		}
	}

	x_t5: #Camera & {
		spec: {
			name:        "FUJIFILM X-T5"
			generation:  "x_trans_v"
			_generation: _

			usb: {
				product_id: 0x02fc
				chunk_size: 16128 * 1024
			}

			features: {
				backup: true

				simulation: {
					slots:    7
					settings: _generation._simulation.settings
					rules:    _generation._simulation.rules
				}

				// TODO: Extract common info to generation
				render: {
					profile_code: 0xff179502
					fields: [
						{id: "head_0"},
						{ref: "file_type"},
						{ref: "image_size"},
						{ref: "image_quality"},
						{ref: "exposure_offset"},
						{ref: "dynamic_range"},
						{ref: "dynamic_range_priority"},
						{ref: "film_simulation"},
						{ref: "grain_effect"},
						{ref: "color_chrome_effect"},
						{id: "white_balance_as_shot"},
						{ref: "white_balance"},
						{ref: "white_balance_shift_red"},
						{ref: "white_balance_shift_blue"},
						{ref: "white_balance_temperature"},
						{ref: "highlight_tone"},
						{ref: "shadow_tone"},
						{ref: "color"},
						{ref: "sharpness"},
						{ref: "noise_reduction"},
						{ref: "lens_modulation_optimizer"},
						{ref: "color_space"},
						{ref: "monochromatic_color_temperature"},
						{ref: "smooth_skin_effect"},
						{ref: "color_chrome_fx_blue"},
						{ref: "monochromatic_color_tint"},
						{ref: "clarity"},
						{ref: "teleconverter"},
						{
							id:        "tail_0"
							skip_read: true
						},
					]
					transformations: [
						{
							when: {ref: "dynamic_range", equals: "hdr800_plus"}
							apply: [
								{ref: "dynamic_range", value: "hdr800"},
								{ref: "dynamic_range_priority", value: "plus"},
							]
						},
						{
							when: {ref: "white_balance", equals: "as_shot"}
							apply: [{ref: "white_balance_as_shot", value: 0x01}]
						},
						{
							when: {not: {ref: "white_balance", equals: "as_shot"}}
							apply: [{ref: "white_balance_as_shot", value: 0x02}]
						},
						{
							apply: [
								{ref: "head_0", value: 0},
								{ref: "tail_0", value: 0},
							]
						},
					]
					rules: [
						{
							message: "Monochromatic color settings only apply to black and white simulations."
							when: all: [
								{any: [
									{ref: "monochromatic_color_temperature", present: true},
									{ref: "monochromatic_color_tint", present: true},
								]},
								{not: {
									ref: "film_simulation"
									in: ["monochrome", "monochrome_ye", "monochrome_r", "monochrome_g", "acros", "acros_ye", "acros_r", "acros_g"]
								}},
							]
						},
						{
							message: "White balance temperature is only meaningful in Temperature mode."
							when: all: [
								{ref: "white_balance_temperature", present: true},
								{not: {ref: "white_balance", equals: "temperature"}},
							]
						},
						{
							message: "White balance shifts apply only when White Balance is not As Shot."
							when: all: [
								{ref: "white_balance", equals: "as_shot"},
								{any: [
									{ref: "white_balance_shift_red", present: true},
									{ref: "white_balance_shift_blue", present: true},
									{ref: "white_balance_temperature", present: true},
								]},
							]
						},
						{
							message: "Dynamic Range cannot exceed the value the image was shot with."
							when: any: [
								{all: [
									{ref: "dynamic_range", scope: "original", equals: "hdr100"},
									{not: {ref: "dynamic_range", in: ["hdr100"]}},
								]},
								{all: [
									{ref: "dynamic_range", scope: "original", equals: "hdr200"},
									{not: {ref: "dynamic_range", in: ["hdr100", "hdr200"]}},
								]},
								{all: [
									{ref: "dynamic_range", scope: "original", equals: "hdr400"},
									{not: {ref: "dynamic_range", in: ["hdr100", "hdr200", "hdr400"]}},
								]},
								{all: [
									{ref: "dynamic_range", scope: "original", equals: "hdr800"},
									{not: {ref: "dynamic_range", in: ["hdr100", "hdr200", "hdr400", "hdr800"]}},
								]},
							]
						},
					]
				}
			}
		}
	}
}
