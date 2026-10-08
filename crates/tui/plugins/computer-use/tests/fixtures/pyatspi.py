"""Synthetic AT-SPI tree: no desktop service or native input is imported."""
import json
import os


STATE_ENABLED = 1
STATE_EDITABLE = 2


class Node:
    def __init__(self, name, children=()):
        self.name = name
        self.children = list(children)
        self.childCount = len(self.children)
        self.nActions = 1
        self.value = "Before"
        self.numeric_value = 0
        self.mode = os.environ.get("CU_ATSPI_EDIT_MODE", "text")

    def getChildAtIndex(self, index):
        return self.children[index]

    def getRoleName(self):
        return "application"

    def getState(self):
        return self

    def contains(self, state):
        return self.mode != ("disabled" if state == STATE_ENABLED else "readonly")

    def queryEditableText(self):
        if self.mode == "numeric":
            raise NotImplementedError()
        if self.mode == "query-failed":
            raise RuntimeError("interface_failed")
        return self

    def record_value(self, value):
        with open(os.environ["CU_ATSPI_ACTIONS"], "a", encoding="utf-8") as output:
            output.write(json.dumps({"name": self.name, "value": value}) + "\n")

    def setTextContents(self, value):
        self.record_value(value)
        self.value = "unexpected" if self.mode == "mismatch" else value
        return self.mode != "rejected"

    def queryText(self):
        return self

    @property
    def characterCount(self):
        return len(self.value)

    def getText(self, start, end):
        return self.value[start:end]

    def queryValue(self):
        if self.mode != "numeric":
            raise NotImplementedError()
        return self

    @property
    def currentValue(self):
        return self.numeric_value

    @currentValue.setter
    def currentValue(self, value):
        self.record_value(value)
        self.numeric_value = value

    def queryAction(self):
        return self

    def getName(self, index):
        return "click"

    def doAction(self, index):
        with open(os.environ["CU_ATSPI_ACTIONS"], "a", encoding="utf-8") as output:
            output.write(json.dumps({"name": self.name, "action": "click"}) + "\n")


class Registry:
    @staticmethod
    def getDesktop(index):
        names = ["Fixture Extended", "Fixture"]
        if os.environ.get("CU_ATSPI_DUPLICATE") == "1":
            names.append("FIXTURE")
        return Node("desktop", [Node(name, [Node("child")]) for name in names])
