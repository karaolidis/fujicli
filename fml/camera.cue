package fml

import "list"

cameras: [string]: #Camera

#Camera: #DefinitionBase & {
	spec: #Spec

	#Spec: {
		name:      string
		sensor:    #RefSensor
		processor: #RefProcessor

		let is = {"sensor": sensor, "processor": processor}
		let matched = [for t in _traits if len([for k, v in t.when if v != is[k] {k}]) == 0 {t}]

		_template: {
			simulation: {
				settings: list.Concat([for t in matched {t.simulation.settings}])
				rules: list.Concat([for t in matched {t.simulation.rules}])
			}
			render: {
				fields: list.Concat([for t in matched {t.render.fields}])
				transformations: list.Concat([for t in matched {t.render.transformations}])
				rules: list.Concat([for t in matched {t.render.rules}])
			}
		}

		usb: #USB

		features?: #Features

		#USB: {
			#FujifilmVendorID:          0x04cb
			#DefaultCameraUSBChunkSize: 1024 * 1024

			vendor_id:  uint16 | *#FujifilmVendorID
			product_id: uint16
			chunk_size: uint & >0 | *#DefaultCameraUSBChunkSize
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

				_unique_ids: list.UniqueItems & [for s in settings {s.id}]
			}

			#Render: {
				fields: [...string]

				_fields: [for f in profile.fields if list.Contains(fields, f.id) {f}]

				#Grammar: #GrammarBase & {
					_scoped: true
					_ids: {
						all: fields
						i: list.Concat([
							[for _, field in _fields if field.ref == _|_ {field.id}],
							[for _, field in _fields if field.ref != _|_ if options[field.ref].spec.kind == "integer" {field.id}],
						])
						f: [for _, field in _fields if field.ref != _|_ if options[field.ref].spec.kind == "float" {field.id}]
						s: [for _, field in _fields if field.ref != _|_ if options[field.ref].spec.kind == "string" {field.id}]
						e: [for _, field in _fields if field.ref != _|_ if options[field.ref].spec.kind == "enum" {field.id}]
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

				_unique_ids: list.UniqueItems & fields
				_known_ids: {for id in fields {(id): list.Contains(_profile_ids, id) & true}}
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
			name:      "FUJIFILM X-E1"
			sensor:    "x_trans_cmos"
			processor: "exr_processor_pro"
			usb: product_id: 0x0283
		}
	}

	x_m1: #Camera & {
		spec: {
			name:      "FUJIFILM X-M1"
			sensor:    "x_trans_cmos"
			processor: "exr_processor_ii"
			usb: product_id: 0x02b6
		}
	}

	x70: #Camera & {
		spec: {
			name:      "FUJIFILM X70"
			sensor:    "x_trans_cmos_ii"
			processor: "exr_processor_ii"
			usb: product_id: 0x02ba
		}
	}

	x_e2: #Camera & {
		spec: {
			name:      "FUJIFILM X-E2"
			sensor:    "x_trans_cmos_ii"
			processor: "exr_processor_ii"
			usb: product_id: 0x02b5
		}
	}

	x_t1: #Camera & {
		spec: {
			name:      "FUJIFILM X-T1"
			sensor:    "x_trans_cmos_ii"
			processor: "exr_processor_ii"
			usb: product_id: 0x02bf
		}
	}

	x_t10: #Camera & {
		spec: {
			name:      "FUJIFILM X-T10"
			sensor:    "x_trans_cmos_ii"
			processor: "exr_processor_ii"
			usb: product_id: 0x02c8
		}
	}

	x100f: #Camera & {
		spec: {
			name:      "FUJIFILM X100F"
			sensor:    "x_trans_cmos_iii"
			processor: "x_processor_pro"
			usb: product_id: 0x02d1
		}
	}

	x_e3: #Camera & {
		spec: {
			name:      "FUJIFILM X-E3"
			sensor:    "x_trans_cmos_iii"
			processor: "x_processor_pro"
			usb: product_id: 0x02d6
		}
	}

	x_h1: #Camera & {
		spec: {
			name:      "FUJIFILM X-H1"
			sensor:    "x_trans_cmos_iii"
			processor: "x_processor_pro"
			usb: product_id: 0x02d7
		}
	}

	x_pro2: #Camera & {
		spec: {
			name:      "FUJIFILM X-Pro2"
			sensor:    "x_trans_cmos_iii"
			processor: "x_processor_pro"
			usb: product_id: 0x02cb
		}
	}

	x_t2: #Camera & {
		spec: {
			name:      "FUJIFILM X-T2"
			sensor:    "x_trans_cmos_iii"
			processor: "x_processor_pro"
			usb: product_id: 0x02cd
		}
	}

	x_t20: #Camera & {
		spec: {
			name:      "FUJIFILM X-T20"
			sensor:    "x_trans_cmos_iii"
			processor: "x_processor_pro"
			usb: product_id: 0x02d4
		}
	}

	x100v: #Camera & {
		spec: {
			name:      "FUJIFILM X100V"
			sensor:    "x_trans_cmos_4"
			processor: "x_processor_4"
			usb: product_id: 0x02e5
		}
	}

	x_e4: #Camera & {
		spec: {
			name:      "FUJIFILM X-E4"
			sensor:    "x_trans_cmos_4"
			processor: "x_processor_4"
			usb: product_id: 0x02e8
		}
	}

	x_pro3: #Camera & {
		spec: {
			name:      "FUJIFILM X-Pro3"
			sensor:    "x_trans_cmos_4"
			processor: "x_processor_4"
			usb: product_id: 0x02e4
		}
	}

	x_s10: #Camera & {
		spec: {
			name:      "FUJIFILM X-S10"
			sensor:    "x_trans_cmos_4"
			processor: "x_processor_4"
			usb: product_id: 0x02ea
		}
	}

	x_t3: #Camera & {
		spec: {
			name:      "FUJIFILM X-T3"
			sensor:    "x_trans_cmos_4"
			processor: "x_processor_4"
			usb: product_id: 0x02dd
		}
	}

	x_t4: #Camera & {
		spec: {
			name:      "FUJIFILM X-T4"
			sensor:    "x_trans_cmos_4"
			processor: "x_processor_4"
			usb: product_id: 0x02e6
		}
	}

	x100vi: #Camera & {
		spec: {
			name:      "FUJIFILM X100VI"
			sensor:    "x_trans_cmos_5_hr"
			processor: "x_processor_5"
			usb: product_id: 0x0305
		}
	}

	x_h2: #Camera & {
		spec: {
			name:      "FUJIFILM X-H2"
			sensor:    "x_trans_cmos_5_hr"
			processor: "x_processor_5"
			_template: _

			_pins: {
				common: [
					{
						message: "Image Size value is not supported by this camera."
						when: not: {ref: "image_size", in: ["l_3_2", "l_16_9", "l_1_1", "l_4_3", "l_5_4", "m_3_2", "m_16_9", "m_1_1", "m_4_3", "m_5_4", "s_3_2", "s_16_9", "s_1_1", "s_4_3", "s_5_4"]}
					},
					{
						message: "Film Simulation value is not supported by this camera."
						when: not: {ref: "film_simulation", in: ["provia", "velvia", "astia", "pro_neg_hi", "pro_neg_std", "monochrome", "monochrome_ye", "monochrome_r", "monochrome_g", "sepia", "classic_chrome", "acros", "acros_ye", "acros_r", "acros_g", "eterna", "classic_negative", "eterna_bleach_bypass", "nostalgic_negative", "reala_ace"]}
					},
					{
						message: "Monochromatic Color Temperature is out of range for this camera."
						when: not: {ref: "monochromatic_color_temperature", min: -18, max: 18}
					},
					{
						message: "Monochromatic Color Tint is out of range for this camera."
						when: not: {ref: "monochromatic_color_tint", min: -18, max: 18}
					},
					{
						message: "Grain Effect value is not supported by this camera."
						when: not: {ref: "grain_effect", in: ["strong_large", "weak_large", "strong_small", "weak_small", "off"]}
					},
					{
						message: "Color Chrome Effect value is not supported by this camera."
						when: not: {ref: "color_chrome_effect", in: ["strong", "weak", "off"]}
					},
					{
						message: "Color Chrome FX Blue value is not supported by this camera."
						when: not: {ref: "color_chrome_fx_blue", in: ["strong", "weak", "off"]}
					},
					{
						message: "White Balance Shift Red is out of range for this camera."
						when: not: {ref: "white_balance_shift_red", min: -9, max: 9}
					},
					{
						message: "White Balance Shift Blue is out of range for this camera."
						when: not: {ref: "white_balance_shift_blue", min: -9, max: 9}
					},
					{
						message: "White Balance Temperature is out of range for this camera."
						when: not: {ref: "white_balance_temperature", min: 2500, max: 10000}
					},
					{
						message: "Highlight Tone is out of range for this camera."
						when: not: {ref: "highlight_tone", min: -2.0, max: 4.0}
					},
					{
						message: "Shadow Tone is out of range for this camera."
						when: not: {ref: "shadow_tone", min: -2.0, max: 4.0}
					},
					{
						message: "Color is out of range for this camera."
						when: not: {ref: "color", min: -4, max: 4}
					},
					{
						message: "Sharpness is out of range for this camera."
						when: not: {ref: "sharpness", min: -4, max: 4}
					},
					{
						message: "Noise Reduction is out of range for this camera."
						when: not: {ref: "noise_reduction", min: -4, max: 4}
					},
					{
						message: "Clarity is out of range for this camera."
						when: not: {ref: "clarity", min: -5, max: 5}
					},
					{
						message: "Lens Modulation Optimizer value is not supported by this camera."
						when: not: {ref: "lens_modulation_optimizer", in: ["on", "off"]}
					},
					{
						message: "Color Space value is not supported by this camera."
						when: not: {ref: "color_space", in: ["srgb", "adobe_rgb"]}
					},
					{
						message: "Dynamic Range value is not supported by this camera."
						when: not: {ref: "dynamic_range", in: ["auto", "hdr100", "hdr200", "hdr400"]}
					},
					{
						message: "Dynamic Range Priority value is not supported by this camera."
						when: not: {ref: "dynamic_range_priority", in: ["auto", "strong", "weak", "off"]}
					},
					{
						message: "Smooth Skin Effect value is not supported by this camera."
						when: not: {ref: "smooth_skin_effect", in: ["strong", "weak", "off"]}
					},
				]
				simulation: [
					{
						message: "Image Quality value is not supported by this camera."
						when: not: {ref: "image_quality", in: ["fine_raw", "fine", "normal_raw", "normal", "raw"]}
					},
					{
						message: "White Balance value is not supported by this camera."
						when: not: {ref: "white_balance", in: ["white_priority", "auto", "ambience_priority", "custom1", "custom2", "custom3", "temperature", "daylight", "shade", "fluorescent1", "fluorescent2", "fluorescent3", "incandescent", "underwater"]}
					},
				]
				render: [
					{
						message: "File Type value is not supported by this camera."
						when: not: {ref: "file_type", in: ["jpeg", "heif", "tiff8", "tiff16"]}
					},
					{
						message: "Image Quality value is not supported by this camera."
						when: not: {ref: "image_quality", in: ["fine", "normal"]}
					},
					{
						message: "Exposure Offset is out of range for this camera."
						when: not: {ref: "exposure_offset", min: -2.0, max: 3.0}
					},
					{
						message: "White Balance value is not supported by this camera."
						when: not: {ref: "white_balance", in: ["as_shot", "white_priority", "auto", "ambience_priority", "custom1", "custom2", "custom3", "temperature", "daylight", "shade", "fluorescent1", "fluorescent2", "fluorescent3", "incandescent", "underwater"]}
					},
					{
						message: "Teleconverter value is not supported by this camera."
						when: not: {ref: "teleconverter", in: ["on", "off"]}
					},
				]
			}

			usb: product_id: 0x02f2

			features: {
				backup: true

				simulation: {
					slots:    7
					settings: _template.simulation.settings
					rules: list.Concat([_pins.common, _pins.simulation, _template.simulation.rules])
				}

				render: {
					fields:          _template.render.fields
					transformations: _template.render.transformations
					rules: list.Concat([_pins.common, _pins.render, _template.render.rules])
				}
			}
		}
	}

	x_h2s: #Camera & {
		spec: {
			name:      "FUJIFILM X-H2S"
			sensor:    "x_trans_cmos_5_hs"
			processor: "x_processor_5"
			usb: product_id: 0x02f0
		}
	}

	x_s20: #Camera & {
		spec: {
			name:      "FUJIFILM X-S20"
			sensor:    "x_trans_cmos_4"
			processor: "x_processor_5"
			_template: _

			_pins: {
				common: [
					{
						message: "Image Size value is not supported by this camera."
						when: not: {ref: "image_size", in: ["l_3_2", "l_16_9", "l_1_1", "m_3_2", "m_16_9", "m_1_1", "s_3_2", "s_16_9", "s_1_1"]}
					},
					{
						message: "Film Simulation value is not supported by this camera."
						when: not: {ref: "film_simulation", in: ["provia", "velvia", "astia", "pro_neg_hi", "pro_neg_std", "monochrome", "monochrome_ye", "monochrome_r", "monochrome_g", "sepia", "classic_chrome", "acros", "acros_ye", "acros_r", "acros_g", "eterna", "classic_negative", "eterna_bleach_bypass", "nostalgic_negative", "reala_ace"]}
					},
					{
						message: "Monochromatic Color Temperature is out of range for this camera."
						when: not: {ref: "monochromatic_color_temperature", min: -18, max: 18}
					},
					{
						message: "Monochromatic Color Tint is out of range for this camera."
						when: not: {ref: "monochromatic_color_tint", min: -18, max: 18}
					},
					{
						message: "Grain Effect value is not supported by this camera."
						when: not: {ref: "grain_effect", in: ["strong_large", "weak_large", "strong_small", "weak_small", "off"]}
					},
					{
						message: "Color Chrome Effect value is not supported by this camera."
						when: not: {ref: "color_chrome_effect", in: ["strong", "weak", "off"]}
					},
					{
						message: "Color Chrome FX Blue value is not supported by this camera."
						when: not: {ref: "color_chrome_fx_blue", in: ["strong", "weak", "off"]}
					},
					{
						message: "White Balance Shift Red is out of range for this camera."
						when: not: {ref: "white_balance_shift_red", min: -9, max: 9}
					},
					{
						message: "White Balance Shift Blue is out of range for this camera."
						when: not: {ref: "white_balance_shift_blue", min: -9, max: 9}
					},
					{
						message: "White Balance Temperature is out of range for this camera."
						when: not: {ref: "white_balance_temperature", min: 2500, max: 10000}
					},
					{
						message: "Highlight Tone is out of range for this camera."
						when: not: {ref: "highlight_tone", min: -2.0, max: 4.0}
					},
					{
						message: "Shadow Tone is out of range for this camera."
						when: not: {ref: "shadow_tone", min: -2.0, max: 4.0}
					},
					{
						message: "Color is out of range for this camera."
						when: not: {ref: "color", min: -4, max: 4}
					},
					{
						message: "Sharpness is out of range for this camera."
						when: not: {ref: "sharpness", min: -4, max: 4}
					},
					{
						message: "Noise Reduction is out of range for this camera."
						when: not: {ref: "noise_reduction", min: -4, max: 4}
					},
					{
						message: "Clarity is out of range for this camera."
						when: not: {ref: "clarity", min: -5, max: 5}
					},
					{
						message: "Lens Modulation Optimizer value is not supported by this camera."
						when: not: {ref: "lens_modulation_optimizer", in: ["on", "off"]}
					},
					{
						message: "Color Space value is not supported by this camera."
						when: not: {ref: "color_space", in: ["srgb", "adobe_rgb"]}
					},
					{
						message: "Dynamic Range value is not supported by this camera."
						when: not: {ref: "dynamic_range", in: ["auto", "hdr100", "hdr200", "hdr400"]}
					},
					{
						message: "Dynamic Range Priority value is not supported by this camera."
						when: not: {ref: "dynamic_range_priority", in: ["auto", "strong", "weak", "off"]}
					},
				]
				simulation: [
					{
						message: "Image Quality value is not supported by this camera."
						when: not: {ref: "image_quality", in: ["fine_raw", "fine", "normal_raw", "normal", "raw"]}
					},
					{
						message: "White Balance value is not supported by this camera."
						when: not: {ref: "white_balance", in: ["white_priority", "auto", "ambience_priority", "custom1", "custom2", "custom3", "temperature", "daylight", "shade", "fluorescent1", "fluorescent2", "fluorescent3", "incandescent", "underwater"]}
					},
				]
				render: [
					{
						message: "File Type value is not supported by this camera."
						when: not: {ref: "file_type", in: ["jpeg", "heif", "tiff8", "tiff16"]}
					},
					{
						message: "Image Quality value is not supported by this camera."
						when: not: {ref: "image_quality", in: ["fine", "normal"]}
					},
					{
						message: "Exposure Offset is out of range for this camera."
						when: not: {ref: "exposure_offset", min: -2.0, max: 3.0}
					},
					{
						message: "White Balance value is not supported by this camera."
						when: not: {ref: "white_balance", in: ["as_shot", "white_priority", "auto", "ambience_priority", "custom1", "custom2", "custom3", "temperature", "daylight", "shade", "fluorescent1", "fluorescent2", "fluorescent3", "incandescent", "underwater"]}
					},
				]
			}

			usb: product_id: 0x02f7

			features: {
				backup: true

				simulation: {
					slots:    4
					settings: _template.simulation.settings
					rules: list.Concat([_pins.common, _pins.simulation, _template.simulation.rules])
				}

				render: {
					fields:          _template.render.fields
					transformations: _template.render.transformations
					rules: list.Concat([_pins.common, _pins.render, _template.render.rules])
				}
			}
		}
	}

	x_t5: #Camera & {
		spec: {
			name:      "FUJIFILM X-T5"
			sensor:    "x_trans_cmos_5_hr"
			processor: "x_processor_5"
			_template: _

			_pins: {
				common: [
					{
						message: "Image Size value is not supported by this camera."
						when: not: {ref: "image_size", in: ["l_3_2", "l_16_9", "l_1_1", "l_4_3", "l_5_4", "m_3_2", "m_16_9", "m_1_1", "m_4_3", "m_5_4", "s_3_2", "s_16_9", "s_1_1", "s_4_3", "s_5_4"]}
					},
					{
						message: "Film Simulation value is not supported by this camera."
						when: not: {ref: "film_simulation", in: ["provia", "velvia", "astia", "pro_neg_hi", "pro_neg_std", "monochrome", "monochrome_ye", "monochrome_r", "monochrome_g", "sepia", "classic_chrome", "acros", "acros_ye", "acros_r", "acros_g", "eterna", "classic_negative", "eterna_bleach_bypass", "nostalgic_negative", "reala_ace"]}
					},
					{
						message: "Monochromatic Color Temperature is out of range for this camera."
						when: not: {ref: "monochromatic_color_temperature", min: -18, max: 18}
					},
					{
						message: "Monochromatic Color Tint is out of range for this camera."
						when: not: {ref: "monochromatic_color_tint", min: -18, max: 18}
					},
					{
						message: "Grain Effect value is not supported by this camera."
						when: not: {ref: "grain_effect", in: ["strong_large", "weak_large", "strong_small", "weak_small", "off"]}
					},
					{
						message: "Color Chrome Effect value is not supported by this camera."
						when: not: {ref: "color_chrome_effect", in: ["strong", "weak", "off"]}
					},
					{
						message: "Color Chrome FX Blue value is not supported by this camera."
						when: not: {ref: "color_chrome_fx_blue", in: ["strong", "weak", "off"]}
					},
					{
						message: "White Balance Shift Red is out of range for this camera."
						when: not: {ref: "white_balance_shift_red", min: -9, max: 9}
					},
					{
						message: "White Balance Shift Blue is out of range for this camera."
						when: not: {ref: "white_balance_shift_blue", min: -9, max: 9}
					},
					{
						message: "White Balance Temperature is out of range for this camera."
						when: not: {ref: "white_balance_temperature", min: 2500, max: 10000}
					},
					{
						message: "Highlight Tone is out of range for this camera."
						when: not: {ref: "highlight_tone", min: -2.0, max: 4.0}
					},
					{
						message: "Shadow Tone is out of range for this camera."
						when: not: {ref: "shadow_tone", min: -2.0, max: 4.0}
					},
					{
						message: "Color is out of range for this camera."
						when: not: {ref: "color", min: -4, max: 4}
					},
					{
						message: "Sharpness is out of range for this camera."
						when: not: {ref: "sharpness", min: -4, max: 4}
					},
					{
						message: "Noise Reduction is out of range for this camera."
						when: not: {ref: "noise_reduction", min: -4, max: 4}
					},
					{
						message: "Clarity is out of range for this camera."
						when: not: {ref: "clarity", min: -5, max: 5}
					},
					{
						message: "Lens Modulation Optimizer value is not supported by this camera."
						when: not: {ref: "lens_modulation_optimizer", in: ["on", "off"]}
					},
					{
						message: "Color Space value is not supported by this camera."
						when: not: {ref: "color_space", in: ["srgb", "adobe_rgb"]}
					},
					{
						message: "Dynamic Range value is not supported by this camera."
						when: not: {ref: "dynamic_range", in: ["auto", "hdr100", "hdr200", "hdr400"]}
					},
					{
						message: "Dynamic Range Priority value is not supported by this camera."
						when: not: {ref: "dynamic_range_priority", in: ["auto", "strong", "weak", "off"]}
					},
					{
						message: "Smooth Skin Effect value is not supported by this camera."
						when: not: {ref: "smooth_skin_effect", in: ["strong", "weak", "off"]}
					},
				]
				simulation: [
					{
						message: "Image Quality value is not supported by this camera."
						when: not: {ref: "image_quality", in: ["fine_raw", "fine", "normal_raw", "normal", "raw"]}
					},
					{
						message: "White Balance value is not supported by this camera."
						when: not: {ref: "white_balance", in: ["white_priority", "auto", "ambience_priority", "custom1", "custom2", "custom3", "temperature", "daylight", "shade", "fluorescent1", "fluorescent2", "fluorescent3", "incandescent", "underwater"]}
					},
				]
				render: [
					{
						message: "File Type value is not supported by this camera."
						when: not: {ref: "file_type", in: ["jpeg", "heif", "tiff8", "tiff16"]}
					},
					{
						message: "Image Quality value is not supported by this camera."
						when: not: {ref: "image_quality", in: ["fine", "normal"]}
					},
					{
						message: "Exposure Offset is out of range for this camera."
						when: not: {ref: "exposure_offset", min: -2.0, max: 3.0}
					},
					{
						message: "White Balance value is not supported by this camera."
						when: not: {ref: "white_balance", in: ["as_shot", "white_priority", "auto", "ambience_priority", "custom1", "custom2", "custom3", "temperature", "daylight", "shade", "fluorescent1", "fluorescent2", "fluorescent3", "incandescent", "underwater"]}
					},
					{
						message: "Teleconverter value is not supported by this camera."
						when: not: {ref: "teleconverter", in: ["on", "off"]}
					},
				]
			}

			usb: {
				product_id: 0x02fc
				chunk_size: 16128 * 1024
			}

			features: {
				backup: true

				simulation: {
					slots:    7
					settings: _template.simulation.settings
					rules: list.Concat([_pins.common, _pins.simulation, _template.simulation.rules])
				}

				render: {
					fields:          _template.render.fields
					transformations: _template.render.transformations
					rules: list.Concat([_pins.common, _pins.render, _template.render.rules])
				}
			}
		}
	}
}
