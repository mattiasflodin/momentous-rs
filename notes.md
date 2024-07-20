https://en.wikipedia.org/wiki/History_of_calendars

https://hea-www.harvard.edu/ECT/Daymarks/

https://en.wikipedia.org/wiki/Proleptic_Gregorian_calendar

# On counting chronologies
Dates (and more so datetimes) are subtly complicated in that they combine two
independent measures into a single value, while creating the illusion of
alignment:
- The position of the Earth in its orbit around the sun
- The position of the Earth in its rotation around its own axis

Consider a date such as 2024-01-13. The year part depends solely on the number
of revolutions that the Earth has made around the Sun. The year increment is
expected to occur at "new year", marking a specific angular position in the
Earth's orbit. In contrast, the month and day components are based on the
Earth's rotation on its axis, incrementing with each complete spin. Since
there's no fixed relationship between the length of the day and the length of
the year, simply counting days to increment years leads to a seasonal drift over
time.

This is why we introduce leap days to shift the new year back to alignment with
the tropical year. By adding leap days the Julian calendar has an amortized year
length of $(3*365 + 366/4) = 365.25 days$. The Gregorian calendar refines this
to $(97*366 + 303*365)/400 = 365.2425$ days. The actual tropical year averages
about 365.24219 days, but it varies due to factors like gravitational influences
from other celestial bodies.

In the Julian calendar era, timekeeping was less precise, relying on the
apparent position of the Sun. A "day" meant the Sun had set and risen again,
without concern for the exact moment of date change. Today, we tie dates to
clocks, switching to a new date at 00:00:00. Since the clock varies across time
zones, the date also changes at different times depending on time zone.

The second-to-second duration from one calendar date to the next varies due to
things like daylight saving time and leap seconds. Daylight saving time can
shorten a day to 23 hours or lengthen it to 25 hours. Leap seconds are added to
keep time aligned with the Earth's rotation. Unlike leap days, they have no
relation to the orbital position of the Earth. But while leap days and leap
seconds serve different purposes, _adding a leap second also slightly delays the
calendar date increment_. Unlike DST this is not compensated for by a later
removal of a second. So the leap second technically corrects a drift in the time
of day by shifting the _date_ in relation to the true orbital position. While
this is of small consequence in relation to other errors caused by the Gregorian
calendar, it illustrates the complex interaction between time and date measures.

At 00:00:00 of December 30 in 2011, Samoa changed its time zone from UTC-10h to
UTC+14h, effectively advancing its local time of day by 24 hours. Media reports
described it as "skipping Friday" as if the day was scratched from the calendar.
However, unlike the Julian-to-Gregorian switch where days were removed, Samoa's
change was an adjustment of the _clock_. If it had been a 23-hour shift, then
2011-12-30 would have been an hour long. With a 24-hour shift, it stands to
reason that the day still occurred in the calendar but had duration of zero.

---

From leap-seconds.list:
```
#       3. The current definition of the relationship between UTC
#       and TAI dates from 1 January 1972. A number of different
#       time scales were in use before that epoch, and it can be
#       quite difficult to compute precise timestamps and time
#       intervals in those "prehistoric" days. For more information,
#       consult:
#
#               The Explanatory Supplement to the Astronomical
#               Ephemeris.
#       or
#               Terry Quinn, "The BIPM and the Accurate Measurement
#               of Time," Proc. of the IEEE, Vol. 79, pp. 894-905,
#               July, 1991. <http://dx.doi.org/10.1109/5.84965>
#               reprinted in:
#                  Christine Hackman and Donald B Sullivan (eds.)
#                  Time and Frequency Measurement
#                  American Association of Physics Teachers (1996)
#                  <http://tf.nist.gov/general/pdf/1168.pdf>, pp. 75-86
```

https://earthsky.org/human-world/friday-december-30-2011-struck-from-samoan-calendar/

# Tom Scott's infamous video on time zones
https://www.youtube.com/watch?v=-5wpm-gesOY

Java datetime has the class Clock to provide current time. Perhaps we can do a similar trait
and implement it e.g. for SystemTime.
https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/time/OffsetDateTime.html#now(java.time.Clock)

Should also check out design of NodaTime since it probably doesn't do everything like JodaTime and perhaps
made some simplifications. In particular the coverage on arithmetic is interesting:
https://nodatime.org/3.1.x/userguide/arithmetic

TODO: there are apparently 10 more leap seconds before 1972 according to leap-seconds.list,
but unclear when they should occur.