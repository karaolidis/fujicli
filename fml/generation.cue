package fml

import "list"

generations: [string]: #Generation

#Generation: #DefinitionBase & {
	spec: #GenerationSpec

	#GenerationSpec: {
		name: string
	}
}

// Ref

#RefGeneration: or([for _, generation in generations {generation.id}])

// Definitions

generations: {
	[Name=string]: id: Name

	// TODO: Investigate generation capabilities
	x_trans: #Generation & {
		spec: name: "X-Trans"
	}

	// TODO: Investigate generation capabilities
	x_trans_ii: #Generation & {
		spec: name: "X-Trans II"
	}

	// TODO: Investigate generation capabilities
	x_trans_iii: #Generation & {
		spec: name: "X-Trans III"
	}

	x_trans_iv: #Generation & {
		spec: {
			name: "X-Trans IV"

			_simulation: {
				settings: [
					{ref: "custom_setting_name"},
					{ref: "image_size"},
					{ref: "image_quality"},
					{ref: "film_simulation"},
					{ref: "monochromatic_color_temperature"},
					{ref: "monochromatic_color_tint"},
					{ref: "grain_effect"},
					{ref: "color_chrome_effect"},
					{ref: "color_chrome_fx_blue"},
					{ref: "white_balance"},
					{ref: "white_balance_shift_red"},
					{ref: "white_balance_shift_blue"},
					{ref: "white_balance_temperature"},
					{ref: "highlight_tone"},
					{ref: "shadow_tone"},
					{ref: "color"},
					{ref: "sharpness"},
					{ref: "noise_reduction"},
					{ref: "clarity"},
					{ref: "lens_modulation_optimizer"},
					{ref: "color_space"},
					{ref: "dynamic_range"},
					{ref: "dynamic_range_priority"},
				]

				rules: [
					{
						message: "Image Size does not apply to RAW image quality."
						when: all: [
							{ref: "image_size", present: true},
							{ref: "image_quality", equals: "raw"},
						]
					},
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
						message: "Color does not apply to black and white or sepia simulations."
						when: all: [
							{ref: "color", present: true},
							{
								ref: "film_simulation"
								in: ["monochrome", "monochrome_ye", "monochrome_r", "monochrome_g", "sepia", "acros", "acros_ye", "acros_r", "acros_g"]
							},
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
						message: "Dynamic Range can only be set when Dynamic Range Priority is disabled."
						when: all: [
							{ref: "dynamic_range", present: true},
							{not: {ref: "dynamic_range_priority", equals: "off"}},
						]
					},
					{
						message: "Highlight Tone can only be set when Dynamic Range Priority is disabled."
						when: all: [
							{ref: "highlight_tone", present: true},
							{not: {ref: "dynamic_range_priority", equals: "off"}},
						]
					},
					{
						message: "Shadow Tone can only be set when Dynamic Range Priority is disabled."
						when: all: [
							{ref: "shadow_tone", present: true},
							{not: {ref: "dynamic_range_priority", equals: "off"}},
						]
					},
				]
			}

			// TODO: Investigate rendering capabilities
		}
	}

	x_trans_v: #Generation & {
		spec: {
			name: "X-Trans V"

			_simulation: {
				settings: list.Concat([x_trans_iv.spec._simulation.settings, [
					{ref: "smooth_skin_effect"},
				]])

				rules: list.Concat([x_trans_iv.spec._simulation.rules, []])
			}

			_render: {
				fields: [
					"file_type",
					"image_size",
					"image_quality",
					"exposure_offset",
					"dynamic_range",
					"dynamic_range_priority",
					"film_simulation",
					"grain_effect",
					"color_chrome_effect",
					"white_balance_as_shot",
					"white_balance",
					"white_balance_shift_red",
					"white_balance_shift_blue",
					"white_balance_temperature",
					"highlight_tone",
					"shadow_tone",
					"color",
					"sharpness",
					"noise_reduction",
					"lens_modulation_optimizer",
					"color_space",
					"monochromatic_color_temperature",
					"smooth_skin_effect",
					"color_chrome_fx_blue",
					"monochromatic_color_tint",
					"clarity",
					"teleconverter",
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
						message: "Color does not apply to black and white or sepia simulations."
						when: all: [
							{ref: "color", present: true},
							{
								ref: "film_simulation"
								in: ["monochrome", "monochrome_ye", "monochrome_r", "monochrome_g", "sepia", "acros", "acros_ye", "acros_r", "acros_g"]
							},
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
						message: "Dynamic Range can only be set when Dynamic Range Priority is disabled."
						when: all: [
							{ref: "dynamic_range", present: true},
							{not: {ref: "dynamic_range_priority", equals: "off"}},
						]
					},
					{
						message: "Highlight Tone can only be set when Dynamic Range Priority is disabled."
						when: all: [
							{ref: "highlight_tone", present: true},
							{not: {ref: "dynamic_range_priority", equals: "off"}},
						]
					},
					{
						message: "Shadow Tone can only be set when Dynamic Range Priority is disabled."
						when: all: [
							{ref: "shadow_tone", present: true},
							{not: {ref: "dynamic_range_priority", equals: "off"}},
						]
					},
					{
						message: "Dynamic Range cannot exceed the value the image was shot with."
						when: any: [
							{all: [
								{ref: "dynamic_range_priority", scope: "original", equals: "weak"},
								{ref: "dynamic_range", in: ["auto", "hdr400", "hdr800", "hdr800_plus"]},
							]},
							{all: [
								{ref: "dynamic_range_priority", scope: "original", equals: "strong"},
								{ref: "dynamic_range", in: ["auto", "hdr800", "hdr800_plus"]},
							]},
							{all: [
								{not: {ref: "dynamic_range_priority", scope: "original", in: ["weak", "strong"]}},
								{any: [
									{all: [
										{ref: "dynamic_range", scope: "original", equals: "hdr100"},
										{ref: "dynamic_range", in: ["auto", "hdr200", "hdr400", "hdr800", "hdr800_plus"]},
									]},
									{all: [
										{ref: "dynamic_range", scope: "original", equals: "hdr200"},
										{ref: "dynamic_range", in: ["auto", "hdr400", "hdr800", "hdr800_plus"]},
									]},
									{all: [
										{ref: "dynamic_range", scope: "original", equals: "hdr400"},
										{ref: "dynamic_range", in: ["auto", "hdr800", "hdr800_plus"]},
									]},
									{all: [
										{ref: "dynamic_range", scope: "original", equals: "hdr800"},
										{ref: "dynamic_range", in: ["auto", "hdr800_plus"]},
									]},
								]},
							]},
						]
					},
					{
						message: "Dynamic Range Priority cannot exceed the value the image was shot with."
						when: any: [
							{all: [
								{ref: "dynamic_range_priority", scope: "original", equals: "off"},
								{ref: "dynamic_range_priority", in: ["auto", "weak", "strong", "plus"]},
							]},
							{all: [
								{ref: "dynamic_range_priority", scope: "original", equals: "weak"},
								{ref: "dynamic_range_priority", in: ["auto", "strong", "plus"]},
							]},
							{all: [
								{ref: "dynamic_range_priority", scope: "original", equals: "strong"},
								{ref: "dynamic_range_priority", in: ["auto", "plus"]},
							]},
							{all: [
								{ref: "dynamic_range_priority", scope: "original", equals: "plus"},
								{ref: "dynamic_range_priority", in: ["auto"]},
							]},
						]
					},
					{
						message: "Teleconverter can only be enabled if it was enabled when the image was shot."
						when: all: [
							{ref: "teleconverter", scope: "original", equals: "off"},
							{ref: "teleconverter", equals: "on"},
						]
					},
				]
			}
		}
	}
}
