#!/usr/bin/env python3
# -*- coding: utf-8 -*-

# coding: utf-8

# type: ignore[import]

# noqa: F401

# fmt: off
# isort:skip_file
# pragma: no cover
# pylint: disable=missing-docstring
# mypy: ignore-errors
# ruff: noqa
# nosec
# flake8: noqa

"""Module docstring."""

import os


def exported(a):
    """Function docstring."""
    return a


def _unexported():
    pass


_DEFAULT = 3
MAX_DEPTH = 8


class Thing:
    """Class docstring."""

    def method(self):
        return 1
