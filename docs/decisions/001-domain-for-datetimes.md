# DR-001: Domain for Datetimes

## Context

Our primary goal library is to promote correctness and safety when dealing with
date and time. As such, we must be rigorous about preventing bugs that could
arise from producing dates outside the domain of a particular representation.

Each possible representation of a date and/or time in the system has some
implicit domain, depending on the data type used to store it. This will not be
consistent across all representations. Converting between these representations
then becomes a risky business, as it's hard to be sure in each situation that
the date is within the domain of the target representation.

It would be nice to have some standardized guarantee about the domain of dates
that can be represented in the library and to ensure that going outside of these
guarantees is not permitted. However, calendars often already have different
domains depending on how they are defined. ISO 8601, for example, is defined
from 0000-01-01 to 9999-12-31; generating a date outside of this domain would
violate the standard. The domain of UNIX timestamps is determined by the
underlying operating system but is typically 64 bits. Additionally, the user may
sometimes desire a small representation for efficiency reasons when they don't
need the full domain, so simply offering maximum flexibility is not always the
best solution.

## Decision

We will define a specific domain for each representation of a date and time in
the library. The domain may sometimes arise from the underlying data type used
to store the date and time, but it may also be limited by the calendar system
being used.

The library will rigorously enforce the domain even when intermediate
representations might sometimes allow for value outside of it. We will not
permit a date outside the defined domain "to be nice" just because we can in a
particular situation.