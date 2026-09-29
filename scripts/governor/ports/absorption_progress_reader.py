"""Port: absorption progress reader — supplies the scorecard and its content identity.

Companion to ``governor.adapters.absorption_progress_reader``. The Governor check
``absorption_progress`` (M514-1lss2s/S01/T04) must decide whether a product
progress scorecard speaks the closed vocabulary in
``data/parse/absorption_progress.schema.yaml`` AND whether the numbers it
publishes are still bound to the bytes that produced them. That needs three
inputs, and this port names them instead of letting the service open files
directly:

* the closed vocabulary (YAML schema);
* the governed scorecard artifact (JSON);
* the recomputed ``sha256`` of every artifact a measurement row binds.

Every method is best-effort and reports absence as ``None``/``False`` instead of
raising: a governor run must not fail because one measurement artifact is
missing. The caller decides whether absence is ``not_measured`` (silence), a
fail-closed violation, or a stale-evidence flag — the port only reports what is
on disk.

Repository-relative only: an absolute path, a drive letter or a ``..`` segment is
not resolvable through this port. That bound is what keeps the scorecard
contract's ``repository_relative`` evidence policy enforceable rather than
aspirational.
"""

from abc import ABC, abstractmethod
from typing import Any, Optional


class AbsorptionProgressReader(ABC):
    """Reads the absorption-progress scorecard and the content it binds."""

    @abstractmethod
    def read_contract(self) -> Optional[dict[str, Any]]:
        """The closed scorecard vocabulary (DATA schema), or ``None`` when absent."""

    @abstractmethod
    def read_scorecard(self) -> Optional[dict[str, Any]]:
        """The governed scorecard document, or ``None`` when it does not exist.

        ``None`` means the measurement system has no committed scorecard yet. It
        is not zero and not compliance: the check treats it as ``not_measured``
        unless the DATA declares the scorecard required.
        """

    @abstractmethod
    def read_json(self, relative_path: str) -> Optional[dict[str, Any]]:
        """A repository-relative JSON document, or ``None`` when unreadable."""

    @abstractmethod
    def file_sha256(self, relative_path: str) -> Optional[str]:
        """``sha256:<64 hex>`` of a repository-relative file, or ``None``.

        ``None`` means the file is absent or unreadable, which the caller reports
        as stale/missing evidence. The value is the form the scorecard contract
        declares (``sha256_format: sha256:<64 hex>``), so the comparison is
        between like forms rather than between a digest and a bare hex string.
        """

    @abstractmethod
    def path_exists(self, relative_path: str) -> bool:
        """True when a repository-relative path exists inside the project root."""


__all__ = ["AbsorptionProgressReader"]
