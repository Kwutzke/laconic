#!/usr/bin/env python3
"""Module docstring."""

# a line comment

MAX_DEPTH = 3
_default_name = "probe"


def exported(a):
    """Function docstring."""
    x = a + 1
    print(x)  # a trailing comment

    # a detached comment

    return x


def _unexported():
    pass


class Thing:
    """Class docstring."""

    def method(self):
        return 1
