import json
from pathlib import Path
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from configure import OVERLAY, atomic_write, boot_config, tuning

class RecoveryConfigTests(unittest.TestCase):
    def test_preserves_boot_settings_and_idempotence(self):
        source = 'verbosity=1\nfdtfile=rockchip/rk3566-radxa-zero3.dtb\noverlays=uart1 spi2\nrootdev=UUID=keep-me\n'
        output = boot_config(source)
        self.assertIn('overlays=uart1 spi2 ' + OVERLAY, output)
        self.assertIn('rootdev=UUID=keep-me', output)
        self.assertEqual(boot_config(output), output)
        self.assertEqual(boot_config('verbosity=1'), 'verbosity=1\noverlays=' + OVERLAY + '\n')

    def test_conflicting_or_ambiguous_configuration_is_rejected(self):
        for value in ('overlays=x\noverlays=y\n', 'overlays=other-imx219\n', 'overlays=ov5647\n'):
            with self.assertRaises(ValueError):
                boot_config(value)

    def test_auto_modes_required_and_only_enum_names_change(self):
        profile = {'scene_isp21': {
            'ae_calib': {'CommCtrl': {'Enable': 1, 'AecOpType': 'RK_AIQ_OP_MODE_AUTO'}},
            'wb_v21': {'control': {'byPass': 0, 'mode': 'CALIB_WB_MODE_AUTO'}},
            'strategy': 'AECV2_STRATEGY_MODE_LOWLIGHT_PRIOR',
            'hdr': 'CALIB_AWB_HDR_FRAME_CHOOSE_MODE_AUTO', 'untouched': [1, 2, 3]}}
        raw = json.dumps(profile).encode()
        changed = tuning(raw)
        self.assertEqual(changed, raw.replace(b'AECV2_STRATEGY_MODE_LOWLIGHT_PRIOR', b'AECV2_STRATEGY_MODE_LOWLIGHT')
                         .replace(b'CALIB_AWB_HDR_FRAME_CHOOSE_MODE_AUTO', b'CALIB_AWB_HDR_FR_CH_AUTO'))
        self.assertEqual(tuning(changed), changed)
        profile['scene_isp21']['ae_calib']['CommCtrl']['Enable'] = 0
        with self.assertRaises(ValueError):
            tuning(json.dumps(profile).encode())
        with self.assertRaises(ValueError):
            tuning(b'{}')

    def test_replace_symlink_without_changing_vendor_file(self):
        with tempfile.TemporaryDirectory() as temp:
            source = Path(temp) / 'vendor.json'
            source.write_bytes(b'original')
            target = Path(temp) / 'configured.json'
            target.symlink_to(source)
            atomic_write(target, b'patched')
            self.assertEqual(source.read_bytes(), b'original')
            self.assertEqual(target.read_bytes(), b'patched')
            self.assertFalse(target.is_symlink())

if __name__ == '__main__':
    unittest.main()
