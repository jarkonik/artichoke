# frozen_string_literal: true

# Tests from IO docs in Ruby 2.6.3
# https://ruby-doc.org/core-2.6.3/IO.html
def spec
  read_testfile
  read_testfile_with_length
  read_testfile_with_length_and_offset
  write
  write_with_offset

  true
end

def read_testfile
  raise unless IO.read("testfile") == "This is line one\nThis is line two\nThis is line three\nAnd so on...\n"
end

def read_testfile_with_length
  raise unless IO.read("testfile", 20) == "This is line one\nThi"
end

def read_testfile_with_length_and_offset
  raise unless IO.read("testfile", 20, 10) == "ne one\nThis is line "
end

def write
  raise unless IO.write("testfile", "0123456789") == 10
  raise unless IO.read("testfile") == "0123456789"
end

def write_with_offset
  raise unless IO.write("testfile2", "0123456789", 20) == 10
  raise unless IO.read("testfile2") == "This is line one\nThi0123456789two\nThis is line three\nAnd so on...\n"
end
