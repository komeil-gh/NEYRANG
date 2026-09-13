"""No-clobber protocol evidence logging."""

import hashlib
import logging
import os


class ProtocolLog(logging.FileHandler):
    def __init__(self, path):
        self.expected = hashlib.sha256()
        self.expected_bytes = 0
        self.failed = False
        super().__init__(path, mode='x', encoding='utf-8')

    def _open(self):
        return open(self.baseFilename, 'x', encoding='utf-8', newline='\n')

    def emit(self, record):
        try:
            payload = (self.format(record) + self.terminator).encode('utf-8')
            self.expected.update(payload)
            self.expected_bytes += len(payload)
            super().emit(record)
        except Exception:
            self.handleError(record)

    def handleError(self, record):
        # Logging runs on the engine I/O thread: retain failure for the owner.
        self.failed = True
        super().handleError(record)

    def verify(self):
        """Call after the engine exits, before publishing a successful shard."""
        with self.lock:
            if self.failed:
                raise OSError('protocol log write failed')
            if self.stream is None:
                raise OSError('protocol log closed before verification')
            self.flush()
            os.fsync(self.stream.fileno())
            with open(self.baseFilename, 'rb') as stream:
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
                size = stream.tell()
            if size != self.expected_bytes or digest != self.expected.hexdigest():
                raise OSError('protocol log integrity mismatch')
            return {'bytes': size, 'sha256': digest}
