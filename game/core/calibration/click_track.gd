# click_track.gd — metrónomo generado en tiempo de ejecución (B9, agente D).
# Un AudioStreamWAV mono de 16 bits con un clic (seno amortiguado) en lead_s + i·period_s.
# El clic i % 4 == 0 es más agudo (acento). El inicio del clic está exactamente en su instante.
class_name ClickTrack
extends RefCounted

const MIX_RATE := 44100
const CLICK_LEN_S := 0.03
const CLICK_DECAY := 150.0      # 1/s
const CLICK_GAIN := 0.8
const FREQ_ACCENT := 1500.0
const FREQ_NORMAL := 1000.0


## Pista de `count` clics. Con loop = true se repite sin fin (usar lead_s = 0 y tail_s = 0 para que
## el bucle mida exactamente count·period_s).
static func make(period_s: float, count: int, lead_s: float = 0.0, tail_s: float = 0.0, loop: bool = false) -> AudioStreamWAV:
	var total_s := lead_s + period_s * float(count) + tail_s
	var frames := maxi(int(round(total_s * MIX_RATE)), 1)
	var data := PackedByteArray()
	data.resize(frames * 2)
	data.fill(0)
	var click_frames := int(CLICK_LEN_S * MIX_RATE)
	for i in range(count):
		var start := int(round((lead_s + period_s * float(i)) * MIX_RATE))
		var freq := FREQ_ACCENT if i % 4 == 0 else FREQ_NORMAL
		for k in range(click_frames):
			var idx := start + k
			if idx >= frames:
				break
			var tt := float(k) / float(MIX_RATE)
			var v := sin(TAU * freq * tt) * exp(-tt * CLICK_DECAY) * CLICK_GAIN
			data.encode_s16(idx * 2, clampi(int(v * 32767.0), -32768, 32767))
	var wav := AudioStreamWAV.new()
	wav.format = AudioStreamWAV.FORMAT_16_BITS
	wav.mix_rate = MIX_RATE
	wav.stereo = false
	wav.data = data
	if loop:
		wav.loop_mode = AudioStreamWAV.LOOP_FORWARD
		wav.loop_begin = 0
		wav.loop_end = frames
	else:
		wav.loop_mode = AudioStreamWAV.LOOP_DISABLED
	return wav


## Instantes (s de canción) de los clics de make(period_s, count, lead_s, ...).
static func click_times(period_s: float, count: int, lead_s: float = 0.0) -> PackedFloat64Array:
	var out := PackedFloat64Array()
	for i in range(count):
		out.append(lead_s + period_s * float(i))
	return out
