package fml

sensors: [string]: #Sensor

#Sensor: #DefinitionBase & {
	spec: #SensorSpec

	#SensorSpec: {
		name: string
	}
}

// Ref

#RefSensor: or([for _, sensor in sensors {sensor.id}])

// Definitions

sensors: {
	[Name=string]: id: Name

	x_trans_cmos: #Sensor & {
		spec: name: "X-Trans CMOS"
	}

	x_trans_cmos_ii: #Sensor & {
		spec: name: "X-Trans CMOS II"
	}

	x_trans_cmos_iii: #Sensor & {
		spec: name: "X-Trans CMOS III"
	}

	x_trans_cmos_4: #Sensor & {
		spec: name: "X-Trans CMOS 4"
	}

	x_trans_cmos_5_hr: #Sensor & {
		spec: name: "X-Trans CMOS 5 HR"
	}

	x_trans_cmos_5_hs: #Sensor & {
		spec: name: "X-Trans CMOS 5 HS"
	}
}
