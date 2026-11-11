package fml

import "list"

profile: #Profile

#Profile: {
	fields: [...#Field]

	#Field: {
		id:   string | *ref
		ref?: #RefOption
	}

	_unique_ids: list.UniqueItems & [for f in fields {f.id}]
}

// Ref

_profile_ids: [for f in profile.fields {f.id}]

// Definitions

profile: fields: [
	{id: "shoot_condition"},
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
	{id: "portrait_enhancer"},
]
