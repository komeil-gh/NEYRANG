import hashlib
import logging
from pathlib import Path
import tempfile
import unittest

from scripts.protocol_log import ProtocolLog


class ProtocolLogTest(unittest.TestCase):
    def test_existing_evidence_is_never_reopened(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'protocol.log'
            path.write_bytes(b'original\n')
            with self.assertRaises(FileExistsError):
                handler = ProtocolLog(path)
                handler.close()
            self.assertEqual(path.read_bytes(), b'original\n')

    def test_exact_bytes_and_corruption_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'protocol.log'
            handler = ProtocolLog(path)
            try:
                handler.setFormatter(logging.Formatter('%(levelname)s:%(message)s'))
                record = logging.LogRecord('chess.engine', logging.DEBUG, '', 0,
                                           ' << %s', ('position نیرنگ',), None)
                handler.handle(record)
                expected = 'DEBUG: << position نیرنگ\n'.encode()
                self.assertEqual(handler.verify(), {
                    'bytes': len(expected), 'sha256': hashlib.sha256(expected).hexdigest()})
                path.write_bytes(b'X' + expected[1:])
                with self.assertRaisesRegex(OSError, 'integrity'):
                    handler.verify()
            finally:
                handler.close()

    def test_failed_write_poisoned_even_if_logging_suppresses_errors(self):
        with tempfile.TemporaryDirectory() as directory:
            handler = ProtocolLog(Path(directory) / 'protocol.log')
            previous = logging.raiseExceptions
            try:
                logging.raiseExceptions = False
                handler.stream.close()
                handler.handle(logging.LogRecord('chess.engine', 10, '', 0, 'lost', (), None))
                with self.assertRaisesRegex(OSError, 'write failed'):
                    handler.verify()
            finally:
                logging.raiseExceptions = previous
                handler.stream = None
                handler.close()


if __name__ == '__main__':
    unittest.main()
