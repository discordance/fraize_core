
pub fn mydsp_faustpower2_f(value: f32) -> f32 {
	(value * value)
}

pub struct mydsp {
	
	fDummy: f32,
	fCheckbox0: f32,
	fHslider0: f32,
	fRec1: [f32;2],
	fHslider1: f32,
	fRec2: [f32;2],
	fVec0: [f32;2],
	fRec0: [f32;2],
	fSamplingFreq: i32,
	
}

impl mydsp {
		
	pub fn new() -> mydsp { 
		mydsp {
			fDummy: 0 as f32,
			fCheckbox0: 0.0,
			fHslider0: 0.0,
			fRec1: [0.0;2],
			fHslider1: 0.0,
			fRec2: [0.0;2],
			fVec0: [0.0;2],
			fRec0: [0.0;2],
			fSamplingFreq: 0,
		}
	}
	
	pub fn metadata(&mut self, m: &mut Meta) { 
		m.declare("author", "JOS, revised by RM");
		m.declare("basics.lib/name", "Faust Basic Element Library");
		m.declare("basics.lib/version", "0.0");
		m.declare("description", "Distortion demo application.");
		m.declare("filename", "distortion");
		m.declare("filters.lib/name", "Faust Filters Library");
		m.declare("filters.lib/version", "0.0");
		m.declare("misceffects.lib/name", "Faust Math Library");
		m.declare("misceffects.lib/version", "2.0");
		m.declare("name", "distortion");
		m.declare("signals.lib/name", "Faust Signal Routing Library");
		m.declare("signals.lib/version", "0.0");
		m.declare("version", "0.0");
	}

	pub fn getSampleRate(&mut self) -> i32 {
		self.fSamplingFreq
	}
	pub fn getNumInputs(&mut self) -> i32 {
		1
	}
	pub fn getNumOutputs(&mut self) -> i32 {
		1
	}
	pub fn getInputRate(&mut self, channel: i32) -> i32 {
		let mut rate: i32;
		match (channel) {
			0 => {
				rate = 1;
				
			},
			_ => {
				rate = -1;
				
			},
			
		} 
		rate
	}
	pub fn getOutputRate(&mut self, channel: i32) -> i32 {
		let mut rate: i32;
		match (channel) {
			0 => {
				rate = 1;
				
			},
			_ => {
				rate = -1;
				
			},
			
		} 
		rate
	}
	
	pub fn classInit(samplingFreq: i32) {
		
	}
	
	pub fn instanceResetUserInterface(&mut self) {
		self.fCheckbox0 = 0.0;
		self.fHslider0 = 0.0;
		self.fHslider1 = 0.0;
		
	}
	
	pub fn instanceClear(&mut self) {
		for l0 in 0..2 {
			self.fRec1[l0 as usize] = 0.0;
			
		}
		for l1 in 0..2 {
			self.fRec2[l1 as usize] = 0.0;
			
		}
		for l2 in 0..2 {
			self.fVec0[l2 as usize] = 0.0;
			
		}
		for l3 in 0..2 {
			self.fRec0[l3 as usize] = 0.0;
			
		}
		
	}
	
	pub fn instanceConstants(&mut self, samplingFreq: i32) {
		self.fSamplingFreq = samplingFreq;
		
	}
	
	pub fn instanceInit(&mut self, samplingFreq: i32) {
		self.instanceConstants(samplingFreq);
		self.instanceResetUserInterface();
		self.instanceClear();
	}
	
	pub fn init(&mut self, samplingFreq: i32) {
		mydsp::classInit(samplingFreq);
		self.instanceInit(samplingFreq);
	}
	
	pub fn buildUserInterface(&mut self, ui_interface: &mut UI<f32>) {
		ui_interface.declare(&mut self.fDummy, "tooltip", "Reference:   https://ccrma.stanford.edu/~jos/pasp/Cubic_Soft_Clipper.html");
		ui_interface.openVerticalBox("CUBIC NONLINEARITY cubicnl");
		ui_interface.declare(&mut self.fCheckbox0, "0", "");
		ui_interface.declare(&mut self.fCheckbox0, "tooltip", "When this is checked, the   nonlinearity has no effect");
		ui_interface.addCheckButton("Bypass", &mut self.fCheckbox0);
		ui_interface.declare(&mut self.fHslider1, "1", "");
		ui_interface.declare(&mut self.fHslider1, "tooltip", "Amount of distortion");
		ui_interface.addHorizontalSlider("Drive", &mut self.fHslider1, 0.0, 0.0, 1.0, 0.01);
		ui_interface.declare(&mut self.fHslider0, "2", "");
		ui_interface.declare(&mut self.fHslider0, "tooltip", "Brings in even harmonics");
		ui_interface.addHorizontalSlider("Offset", &mut self.fHslider0, 0.0, 0.0, 1.0, 0.01);
		ui_interface.closeBox();
		
	}
	
	pub fn compute(&mut self, count: i32, inputs: &[&[f32]], outputs: &mut[&mut[f32]]) {
		let mut iSlow0: i32 = ((self.fCheckbox0 as f32) as i32);
		let mut fSlow1: f32 = (0.00100000005 * (self.fHslider0 as f32));
		let mut fSlow2: f32 = (0.00100000005 * (self.fHslider1 as f32));
		for i in 0..count {
			self.fRec1[0] = (fSlow1 + (0.999000013 * self.fRec1[1]));
			let mut fTemp0: f32 = (inputs[0][i as usize] as f32);
			self.fRec2[0] = (fSlow2 + (0.999000013 * self.fRec2[1]));
			let mut fTemp1: f32 = f32::max(-1.0, f32::min(1.0, (self.fRec1[0] + (if (iSlow0 as i32 == 1) { 0.0 } else { fTemp0 } * f32::powf(10.0, (2.0 * self.fRec2[0]))))));
			let mut fTemp2: f32 = (fTemp1 * (1.0 - (0.333333343 * mydsp_faustpower2_f(fTemp1))));
			self.fVec0[0] = fTemp2;
			self.fRec0[0] = (((0.995000005 * self.fRec0[1]) + fTemp2) - self.fVec0[1]);
			outputs[0][i as usize] = (if (iSlow0 as i32 == 1) { fTemp0 } else { self.fRec0[0] } as f32);
			self.fRec1[1] = self.fRec1[0];
			self.fRec2[1] = self.fRec2[0];
			self.fVec0[1] = self.fVec0[0];
			self.fRec0[1] = self.fRec0[0];
			
		}
		
	}

}
