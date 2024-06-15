        // TODO store leap seconds reference in chronology object so we don't have to take
        // a lock each time we fetch it, and don't get unpredictable handling of leap seconds.
        // If the leap second table is updated, it should be incorporated into the chronology
        // at a deterministic point, not whenever the table is fetched.