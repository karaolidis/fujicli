package fml

import "list"

import "strconv"

options: [string]: #Option

#Option: #DefinitionBase & {
	spec:     #Spec
	codegen?: #Codegen

	#Spec: #SpecInteger | #SpecFloat | #SpecString | #SpecEnum

	#SpecBase: {
		name:     string
		kind:     #SpecKind
		rules?:   _
		encoding: _

		#SpecKind: "integer" | "float" | "string" | "enum"
	}

	#SpecEncodingBase: {
		prop_code?: uint16
		kind:       string
		spec?:      _
	}

	#SpecInteger: #SpecBase & {
		kind:     "integer"
		rules?:   #Rules
		encoding: #Encoding

		#Rules: {
			min?:  int
			max?:  int
			step?: int

			if min != _|_ && max != _|_ {
				min: <=max
			}
		}

		#Encoding:
			#EncodingRaw |
			#EncodingScale |
			#EncodingLookup

		#EncodingRaw: #SpecEncodingBase & {
			kind: "raw"
		}

		#EncodingScale: #SpecEncodingBase & {
			kind: "scale"
			spec: #Scale

			#Scale: {
				scale: int
			}
		}

		#EncodingLookup: #SpecEncodingBase & {
			kind: "lookup"
			spec: #Lookup

			#Lookup: {
				values: {
					[string]: int | [int, ...int]
				}

				_validation: {
					for k, _ in values {
						let i = strconv.ParseInt(k, 10, 64)
						"\(k)": int & i

						if rules != _|_ {
							if rules.min != _|_ {"\(k)": >=rules.min}
							if rules.max != _|_ {"\(k)": <=rules.max}
						}
					}
				}
			}
		}
	}

	#SpecFloat: #SpecBase & {
		kind:     "float"
		rules?:   #Rules
		encoding: #Encoding

		#Rules: {
			min?:  float
			max?:  float
			step?: float

			if min != _|_ && max != _|_ {
				min: <=max
			}
		}

		#Encoding:
			#EncodingRaw |
			#EncodingScale |
			#EncodingLookup

		#EncodingRaw: #SpecEncodingBase & {
			kind: "raw"
		}

		#EncodingScale: #SpecEncodingBase & {
			kind: "scale"
			spec: #Scale

			#Scale: {
				scale: int
			}
		}

		#EncodingLookup: #SpecEncodingBase & {
			kind: "lookup"
			spec: #Lookup

			#Lookup: {
				values: {
					[string]: int | [int, ...int]
				}

				_validation: {
					for k, _ in values {
						let f = strconv.ParseFloat(k, 64)
						"\(k)": float & f

						if rules != _|_ {
							if rules.min != _|_ {"\(k)": >=rules.min}
							if rules.max != _|_ {"\(k)": <=rules.max}
						}
					}
				}
			}
		}
	}

	#SpecString: #SpecBase & {
		kind:     "string"
		rules?:   #Rules
		encoding: #Encoding

		#Rules: {
			min_length?: uint
			max_length?: uint

			if min_length != _|_ && max_length != _|_ {
				min_length: <=max_length
			}
		}

		#Encoding: #EncodingRaw

		#EncodingRaw: #SpecEncodingBase & {
			kind: "raw"
		}
	}

	#SpecEnum: #SpecBase & {
		kind:     "enum"
		rules:    #Rules
		encoding: #Encoding

		#Rules: {
			variants: [...#Variant]

			#Variant: {
				id:   string
				name: string
				aliases: [...string]
			}

			_validation: {
				ids: list.UniqueItems & [for v in variants {v.id}]
				aliases: list.UniqueItems & [for v in variants for a in v.aliases {a}]
			}
		}

		#Encoding: #EncodingLookup

		#EncodingLookup: #SpecEncodingBase & {
			kind: "lookup"
			spec: #Lookup

			#Lookup: {
				values: close({
					for v in rules.variants {
						"\(v.id)": int | [int, ...int]
					}
				})
			}
		}
	}

	#Codegen: {
		skip_args?: true
	}
}

// Ref

#RefOption: or([for _, option in options {option.id}])

// Definitions

options: {
	[Name=string]: id: Name

	custom_setting: #Option & {
		spec: {
			name: "Custom Setting Slot"
			kind: "enum"
			rules: variants: [
				{id: "c1", name: "C1", aliases: ["c1", "1"]},
				{id: "c2", name: "C2", aliases: ["c2", "2"]},
				{id: "c3", name: "C3", aliases: ["c3", "3"]},
				{id: "c4", name: "C4", aliases: ["c4", "4"]},
				{id: "c5", name: "C5", aliases: ["c5", "5"]},
				{id: "c6", name: "C6", aliases: ["c6", "6"]},
				{id: "c7", name: "C7", aliases: ["c7", "7"]},
			]
			encoding: {
				prop_code: 0xD18C
				kind:      "lookup"
				spec: values: {
					c1: 0x1
					c2: 0x2
					c3: 0x3
					c4: 0x4
					c5: 0x5
					c6: 0x6
					c7: 0x7
				}
			}
		}
		codegen: skip_args: true
	}

	usb_mode: #Option & {
		spec: {
			name: "USB Mode"
			kind: "enum"
			rules: variants: [
				{id: "raw_conversion", name: "Raw Conversion", aliases: ["raw", "rawconversion"]},
			]
			encoding: {
				prop_code: 0xD16E
				kind:      "lookup"
				spec: values: {
					raw_conversion: 0x6
				}
			}
		}
		codegen: skip_args: true
	}

	custom_setting_name: #Option & {
		spec: {
			name: "Custom Setting Name"
			kind: "string"
			rules: {max_length: 25}
			encoding: {
				prop_code: 0xD18D
				kind:      "raw"
			}
		}
	}

	image_size: #Option & {
		spec: {
			name: "Image Size"
			kind: "enum"
			rules: variants: [
				{id: "7728x5152", name: "7728x5152", aliases: ["7728x5152"]},
				{id: "7728x4344", name: "7728x4344", aliases: ["7728x4344"]},
				{id: "5152x5152", name: "5152x5152", aliases: ["5152x5152"]},
				{id: "6864x5152", name: "6864x5152", aliases: ["6864x5152"]},
				{id: "6432x5152", name: "6432x5152", aliases: ["6432x5152"]},
				{id: "5472x3648", name: "5472x3648", aliases: ["5472x3648"]},
				{id: "5472x3080", name: "5472x3080", aliases: ["5472x3080"]},
				{id: "3648x3648", name: "3648x3648", aliases: ["3648x3648"]},
				{id: "4864x3648", name: "4864x3648", aliases: ["4864x3648"]},
				{id: "4560x3648", name: "4560x3648", aliases: ["4560x3648"]},
				{id: "3888x2592", name: "3888x2592", aliases: ["3888x2592"]},
				{id: "3888x2184", name: "3888x2184", aliases: ["3888x2184"]},
				{id: "2592x2592", name: "2592x2592", aliases: ["2592x2592"]},
				{id: "3456x2592", name: "3456x2592", aliases: ["3456x2592"]},
				{id: "3264x2592", name: "3264x2592", aliases: ["3264x2592"]},
			]
			encoding: {
				prop_code: 0xD18E
				kind:      "lookup"
				spec: values: {
					"7728x5152": 0x07
					"7728x4344": 0x08
					"5152x5152": 0x09
					"6864x5152": 0x0e
					"6432x5152": 0x10
					"5472x3648": 0x04
					"5472x3080": 0x05
					"3648x3648": 0x06
					"4864x3648": 0x12
					"4560x3648": 0x14
					"3888x2592": 0x01
					"3888x2184": 0x02
					"2592x2592": 0x03
					"3456x2592": 0x0a
					"3264x2592": 0x0c
				}

			}
		}
	}

	image_quality: #Option & {
		spec: {
			name: "Image Quality"
			kind: "enum"
			rules: variants: [
				{id: "fine_raw", name: "Fine + RAW", aliases: ["fineraw"]},
				{id: "fine", name: "Fine", aliases: ["fine"]},
				{id: "normal_raw", name: "Normal + RAW", aliases: ["normalraw"]},
				{id: "normal", name: "Normal", aliases: ["normal"]},
				{id: "raw", name: "RAW", aliases: ["raw"]},
			]
			encoding: {
				prop_code: 0xD18F
				kind:      "lookup"
				spec: values: {
					fine_raw:   0x04
					fine:       0x02
					normal_raw: 0x05
					normal:     0x03
					raw:        0x01
				}
			}
		}
	}

	film_simulation: #Option & {
		spec: {
			name: "Film Simulation"
			kind: "enum"
			rules: variants: [
				{id: "provia", name: "Provia", aliases: ["provia"]},
				{id: "velvia", name: "Velvia", aliases: ["velvia"]},
				{id: "astia", name: "Astia", aliases: ["astia"]},
				{id: "pro_neg_hi", name: "PRO Neg. Hi", aliases: ["proneghi", "proneghigh"]},
				{id: "pro_neg_std", name: "PRO Neg. Std", aliases: ["pronegstd", "pronegstandard"]},
				{id: "monochrome", name: "Monochrome", aliases: ["mono", "monochrome"]},
				{id: "monochrome_ye", name: "Monochrome + Ye", aliases: ["monoy", "monoye", "monoyellow", "monochromey", "monochromeye", "monochromeyellow"]},
				{id: "monochrome_r", name: "Monochrome + R", aliases: ["monor", "monored", "monochromer", "monochromered"]},
				{id: "monochrome_g", name: "Monochrome + G", aliases: ["monog", "monogreen", "monochromeg", "monochromegreen"]},
				{id: "sepia", name: "Sepia", aliases: ["sepia"]},
				{id: "classic_chrome", name: "Classic Chrome", aliases: ["classicchrome"]},
				{id: "acros", name: "Acros", aliases: ["acros"]},
				{id: "acros_ye", name: "Acros + Ye", aliases: ["acrosy", "acrosye", "acrosyellow"]},
				{id: "acros_r", name: "Acros + R", aliases: ["acrosr", "acrosred"]},
				{id: "acros_g", name: "Acros + G", aliases: ["acrosg", "acrosgreen"]},
				{id: "eterna", name: "Eterna", aliases: ["eterna"]},
				{id: "classic_negative", name: "Classic Negative", aliases: ["classicneg", "classicnegative"]},
				{id: "eterna_bleach_bypass", name: "Eterna Bleach Bypass", aliases: ["eternabb", "eternableach", "eternableachbypass"]},
				{id: "nostalgic_negative", name: "Nostalgic Negative", aliases: ["nostalgicneg", "nostalgicnegative"]},
				{id: "reala_ace", name: "Reala Ace", aliases: ["realaace", "reala"]},
			]
			encoding: {
				prop_code: 0xD192
				kind:      "lookup"
				spec: values: {
					provia:               0x01
					velvia:               0x02
					astia:                0x03
					pro_neg_hi:           0x04
					pro_neg_std:          0x05
					monochrome:           0x06
					monochrome_ye:        0x07
					monochrome_r:         0x08
					monochrome_g:         0x09
					sepia:                0x0a
					classic_chrome:       0x0b
					acros:                0x0c
					acros_ye:             0x0d
					acros_r:              0x0e
					acros_g:              0x0f
					eterna:               0x10
					classic_negative:     0x11
					eterna_bleach_bypass: 0x12
					nostalgic_negative:   0x13
					reala_ace:            0x14
				}
			}
		}
	}

	monochromatic_color_temperature: #Option & {
		spec: {
			name: "Monochromatic Color Temperature"
			kind: "integer"
			rules: {min: -18, max: 18, step: 1}
			encoding: {
				prop_code: 0xD193
				kind:      "scale"
				spec: scale: 10
			}
		}
	}

	monochromatic_color_tint: #Option & {
		spec: {
			name: "Monochromatic Color Tint"
			kind: "integer"
			rules: {min: -18, max: 18, step: 1}
			encoding: {
				prop_code: 0xD194
				kind:      "scale"
				spec: scale: 10
			}
		}
	}

	grain_effect: #Option & {
		spec: {
			name: "Grain Effect"
			kind: "enum"
			rules: variants: [
				{id: "strong_large", name: "Strong Large", aliases: ["stronglarge", "largestrong"]},
				{id: "weak_large", name: "Weak Large", aliases: ["weaklarge", "largeweak"]},
				{id: "strong_small", name: "Strong Small", aliases: ["strongsmall", "smallstrong"]},
				{id: "weak_small", name: "Weak Small", aliases: ["weaksmall", "smallweak"]},
				{id: "off", name: "Off", aliases: ["off"]},
			]
			encoding: {
				prop_code: 0xD195
				kind:      "lookup"
				spec: values: {
					strong_large: 0x05
					weak_large:   0x04
					strong_small: 0x03
					weak_small:   0x02
					off:          0x01
				}
			}
		}
	}

	color_chrome_effect: #Option & {
		spec: {
			name: "Color Chrome Effect"
			kind: "enum"
			rules: variants: [
				{id: "strong", name: "Strong", aliases: ["strong"]},
				{id: "weak", name: "Weak", aliases: ["weak"]},
				{id: "off", name: "Off", aliases: ["off"]},
			]
			encoding: {
				prop_code: 0xD196
				kind:      "lookup"
				spec: values: {strong: 0x03, weak: 0x02, off: 0x01}
			}
		}
	}

	color_chrome_fx_blue: #Option & {
		spec: {
			name: "Color Chrome FX Blue"
			kind: "enum"
			rules: variants: [
				{id: "strong", name: "Strong", aliases: ["strong"]},
				{id: "weak", name: "Weak", aliases: ["weak"]},
				{id: "off", name: "Off", aliases: ["off"]},
			]
			encoding: {
				prop_code: 0xD197
				kind:      "lookup"
				spec: values: {strong: 0x03, weak: 0x02, off: 0x01}
			}
		}
	}

	white_balance: #Option & {
		spec: {
			name: "White Balance"
			kind: "enum"
			rules: variants: [
				{id: "as_shot", name: "As Shot", aliases: ["asshot", "original"]},
				{id: "white_priority", name: "White Priority", aliases: ["whitepriority", "white"]},
				{id: "auto", name: "Auto", aliases: ["auto"]},
				{id: "ambience_priority", name: "Ambience Priority", aliases: ["ambiencepriority", "ambience", "ambient"]},
				{id: "custom1", name: "Custom 1", aliases: ["custom1", "c1"]},
				{id: "custom2", name: "Custom 2", aliases: ["custom2", "c2"]},
				{id: "custom3", name: "Custom 3", aliases: ["custom3", "c3"]},
				{id: "temperature", name: "Temperature", aliases: ["temperature", "temp", "k", "kelvin"]},
				{id: "daylight", name: "Daylight", aliases: ["daylight", "sunny"]},
				{id: "shade", name: "Shade", aliases: ["shade", "cloudy"]},
				{id: "fluorescent1", name: "Fluorescent 1", aliases: ["fluorescent1"]},
				{id: "fluorescent2", name: "Fluorescent 2", aliases: ["fluorescent2"]},
				{id: "fluorescent3", name: "Fluorescent 3", aliases: ["fluorescent3"]},
				{id: "incandescent", name: "Incandescent", aliases: ["incandescent", "tungsten"]},
				{id: "underwater", name: "Underwater", aliases: ["underwater"]},
			]
			encoding: {
				prop_code: 0xD199
				kind:      "lookup"
				spec: values: {
					as_shot:           0x0000
					white_priority:    0x8020
					auto:              0x0002
					ambience_priority: 0x8021
					custom1:           0x8008
					custom2:           0x8009
					custom3:           0x800a
					temperature:       0x8007
					daylight:          0x0004
					shade:             0x8006
					fluorescent1:      0x8001
					fluorescent2:      0x8002
					fluorescent3:      0x8003
					incandescent:      0x0006
					underwater:        0x0008
				}
			}
		}
	}

	white_balance_shift_red: #Option & {
		spec: {
			name: "White Balance Shift Red"
			kind: "integer"
			rules: {min: -9, max: 9, step: 1}
			encoding: {
				prop_code: 0xD19A
				kind:      "scale"
				spec: scale: 1
			}
		}
	}

	white_balance_shift_blue: #Option & {
		spec: {
			name: "White Balance Shift Blue"
			kind: "integer"
			rules: {min: -9, max: 9, step: 1}
			encoding: {
				prop_code: 0xD19B
				kind:      "scale"
				spec: scale: 1
			}
		}
	}

	white_balance_temperature: #Option & {
		spec: {
			name: "White Balance Temperature"
			kind: "integer"
			rules: {min: 2500, max: 10000, step: 10}
			encoding: {
				prop_code: 0xD19C
				kind:      "scale"
				spec: scale: 1
			}
		}
	}

	highlight_tone: #Option & {
		spec: {
			name: "Highlight Tone"
			kind: "float"
			rules: {min: -2.0, max: 4.0, step: 0.5}
			encoding: {
				prop_code: 0xD19D
				kind:      "scale"
				spec: scale: 10
			}
		}
	}

	shadow_tone: #Option & {
		spec: {
			name: "Shadow Tone"
			kind: "float"
			rules: {min: -2.0, max: 4.0, step: 0.5}
			encoding: {
				prop_code: 0xD19E
				kind:      "scale"
				spec: scale: 10
			}
		}
	}

	color: #Option & {
		spec: {
			name: "Color"
			kind: "integer"
			rules: {min: -4, max: 4, step: 1}
			encoding: {
				prop_code: 0xD19F
				kind:      "scale"
				spec: scale: 10
			}
		}
	}

	sharpness: #Option & {
		spec: {
			name: "Sharpness"
			kind: "integer"
			rules: {min: -4, max: 4, step: 1}
			encoding: {
				prop_code: 0xD1A0
				kind:      "scale"
				spec: scale: 10
			}
		}
	}

	noise_reduction: #Option & {
		spec: {
			name: "High ISO Noise Reduction"
			kind: "integer"
			rules: {min: -4, max: 4, step: 1}
			encoding: {
				prop_code: 0xD1A1
				kind:      "lookup"
				spec: values: {
					"4":  0x5000
					"3":  0x6000
					"2":  0x0000
					"1":  0x1000
					"0":  0x2000
					"-1": 0x3000
					"-2": 0x4000
					"-3": 0x7000
					"-4": 0x8000
				}
			}
		}
	}

	clarity: #Option & {
		spec: {
			name: "Clarity"
			kind: "integer"
			rules: {min: -5, max: 5, step: 1}
			encoding: {
				prop_code: 0xD1A2
				kind:      "scale"
				spec: scale: 10
			}
		}
	}

	lens_modulation_optimizer: #Option & {
		spec: {
			name: "Lens Modulation Optimizer"
			kind: "enum"
			rules: {
				variants: [
					{id: "on", name: "On", aliases: ["on", "true"]},
					{id: "off", name: "Off", aliases: ["off", "false"]},
				]
			}
			encoding: {
				prop_code: 0xD1A3
				kind:      "lookup"
				spec: values: {on: 0x01, off: 0x02}
			}
		}
	}

	color_space: #Option & {
		spec: {
			name: "Color Space"
			kind: "enum"
			rules: variants: [
				{id: "srgb", name: "sRGB", aliases: ["s", "srgb"]},
				{id: "adobe_rgb", name: "Adobe RGB", aliases: ["adobe", "adobergb"]},
			]
			encoding: {
				prop_code: 0xD1A4
				kind:      "lookup"
				spec: values: {srgb: 0x02, adobe_rgb: 0x01}
			}
		}
	}

	dynamic_range: #Option & {
		spec: {
			name: "Dynamic Range"
			kind: "enum"
			rules: variants: [
				{id: "auto", name: "Auto", aliases: ["auto"]},
				{id: "hdr100", name: "HDR100", aliases: ["100", "hdr100", "dr100"]},
				{id: "hdr200", name: "HDR200", aliases: ["200", "hdr200", "dr200"]},
				{id: "hdr400", name: "HDR400", aliases: ["400", "hdr400", "dr400"]},
				{id: "hdr800", name: "HDR800", aliases: ["800", "hdr800", "dr800"]},
				{id: "hdr800_plus", name: "HDR800+", aliases: ["800+", "hdr800+", "hdr800plus", "dr800+", "dr800plus"]},
			]
			encoding: {
				prop_code: 0xD190
				kind:      "lookup"
				spec: values: {
					auto:        0xffff
					hdr100:      0x64
					hdr200:      0xc8
					hdr400:      0x190
					hdr800:      0x320
					hdr800_plus: 0x640
				}
			}
		}
	}

	dynamic_range_priority: #Option & {
		spec: {
			name: "Dynamic Range Priority"
			kind: "enum"
			rules: variants: [
				{id: "auto", name: "Auto", aliases: ["auto"]},
				{id: "plus", name: "Plus", aliases: ["plus"]},
				{id: "strong", name: "Strong", aliases: ["strong"]},
				{id: "weak", name: "Weak", aliases: ["weak"]},
				{id: "off", name: "Off", aliases: ["off"]},
			]
			encoding: {
				prop_code: 0xD191
				kind:      "lookup"
				spec: values: {
					auto:   0x8000
					plus:   0x03
					strong: 0x02
					weak:   0x01
					off:    0x00
				}
			}
		}
	}

	smooth_skin_effect: #Option & {
		spec: {
			name: "Smooth Skin Effect"
			kind: "enum"
			rules: variants: [
				{id: "strong", name: "Strong", aliases: ["strong"]},
				{id: "weak", name: "Weak", aliases: ["weak"]},
				{id: "off", name: "Off", aliases: ["off"]},
			]
			encoding: {
				prop_code: 0xD198
				kind:      "lookup"
				spec: values: {
					strong: 0x03
					weak:   0x02
					off:    0x01
				}
			}
		}
	}

	exposure_offset: #Option & {
		spec: {
			name: "Exposure Offset"
			kind: "float"
			rules: {min: -3.0, max: 3.0}
			encoding: {
				kind: "lookup"
				spec: values: {
					"3.0":  3000
					"2.7":  2667
					"2.3":  2333
					"2.0":  2000
					"1.7":  1667
					"1.3":  1333
					"1.0":  1000
					"0.7":  667
					"0.3":  333
					"0.0":  0
					"-0.3": -333
					"-0.7": -667
					"-1.0": -1000
					"-1.3": -1333
					"-1.7": -1667
					"-2.0": -2000
					"-2.3": -2333
					"-2.7": -2667
					"-3.0": -3000
				}
			}
		}
	}

	file_type: #Option & {
		spec: {
			name: "File Type"
			kind: "enum"
			rules: variants: [
				{id: "jpeg", name: "JPEG", aliases: ["jpeg", "jpg"]},
				{id: "heif", name: "HEIF", aliases: ["heif"]},
				{id: "tiff8", name: "TIFF 8-bit", aliases: ["tiff8"]},
				{id: "tiff16", name: "TIFF 16-bit", aliases: ["tiff16"]},
			]
			encoding: {
				kind: "lookup"
				spec: values: {
					jpeg:   0x07
					heif:   0x12
					tiff8:  0x09
					tiff16: 0x0b
				}
			}
		}
	}

	teleconverter: #Option & {
		spec: {
			name: "Teleconverter"
			kind: "enum"
			rules: variants: [
				{id: "on", name: "On", aliases: ["on", "true"]},
				{id: "off", name: "Off", aliases: ["off", "false"]},
			]
			encoding: {
				kind: "lookup"
				spec: values: {
					on:  0x01
					off: 0x02
				}
			}
		}
	}
}
