"""Helper code to run tests."""

from collections import defaultdict
from typing import Any

from torc.api import DefaultApi, iter_documents


class DatabaseInterface:
    """Contains helper code to access objects from the database in tests."""

    def __init__(self, api: DefaultApi, workflow: Any):
        """Initialize the database interface.

        Parameters
        ----------
        api : DefaultApi
            OpenAPI client for the Torc database.
        workflow : Any
            Workflow object.
        """
        self._api = api
        self._workflow = workflow
        self._names_to_ids = self._map_names_to_ids(api, workflow.id)

    @staticmethod
    def _map_names_to_ids(api: DefaultApi, workflow_id: int) -> dict[str, dict[str, int]]:
        """Map document names to IDs for all document types."""
        doc_types = (
            "files",
            "jobs",
            "local_schedulers",
            "resource_requirements",
            "slurm_schedulers",
            "user_data",
        )
        lookup: dict[str, dict[str, int]] = defaultdict(dict)
        for doc_type in doc_types:
            method = getattr(api, f"list_{doc_type}")
            for doc in iter_documents(method, workflow_id):
                assert doc.name not in lookup[doc_type], f"{doc_type=} {doc.name=}"
                lookup[doc_type][doc.name] = doc.id
        return lookup

    @property
    def api(self) -> DefaultApi:
        """Return the API object."""
        return self._api

    @property
    def workflow(self) -> Any:
        """Return the workflow object."""
        return self._workflow

    def get_document(self, document_type: str, name: str) -> Any:
        """Return the document from the API by first mapping the name.

        Parameters
        ----------
        document_type : str
            Type of document.
        name : str
            Name of the document.

        Returns
        -------
        Any
            The document object.
        """
        if document_type in {"resource_requirements", "user_data"}:
            get_one = f"get_{document_type}"
        else:
            get_one = f"get_{document_type[:-1]}"
        method = getattr(self._api, get_one)
        return method(self._names_to_ids[document_type][name])

    def get_document_id(self, document_type: str, name: str) -> int:
        """Return the ID for name.

        Parameters
        ----------
        document_type : str
            Type of document.
        name : str
            Name of the document.

        Returns
        -------
        int
            Document ID.
        """
        return self._names_to_ids[document_type][name]

    def list_documents(self, document_type: str) -> list[Any]:
        """Return all documents of the given type.

        Parameters
        ----------
        document_type : str
            Type of document.

        Returns
        -------
        list[Any]
            List of documents.
        """
        method = getattr(self._api, f"list_{document_type}")
        return list(iter_documents(method, self._workflow.id))

    @property
    def url(self) -> str:
        """Return the database URL."""
        return self._api.api_client.configuration.host
